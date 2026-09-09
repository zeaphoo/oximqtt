//! Unit tests for the client task: endpoint/client-id pattern resolution,
//! the publish window bookkeeping and the graceful stop path.

use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::time;

use super::{on_incoming, Outbound};
use crate::client::{wait_stopped, Ctx};
use crate::mqtt::Packet;
use crate::options::PayloadBuilder;
use crate::options::{Opts, Protocol};
use crate::packets::Incoming;

fn c_args(qos: u8, max_inflight: usize, max_limit: u64) -> Opts {
    Opts {
        conns: 4,
        qos,
        max_inflight,
        max_limit,
        pub_switch: true,
        pub_interval: 0,
        topic_pattern: "t/{no}".into(),
        topic_no_range: vec![0, 3],
        size: 32,
        ..Default::default()
    }
}

fn ctx(qos: u8, max_inflight: usize, max_limit: u64) -> Arc<Ctx> {
    Arc::new(Ctx::for_test(
        c_args(qos, max_inflight, max_limit),
        Protocol::V3,
    ))
}

#[test]
fn client_id_and_endpoints_come_from_the_patterns() {
    let args = Opts {
        conns: 2,
        id_pattern: "c-{no}".into(),
        topic_pattern: "t/{no}/x".into(),
        ..Default::default()
    };
    let c = Ctx::for_test(args, Protocol::V3);
    assert_eq!(c.client_id(7), "c-7");
    assert_eq!(c.endpoints(3).0, c.peers[0]);
    assert_eq!(c.binds.len(), 0);

    c.kick(1);
    assert!(c.take_kick(1));
    assert!(!c.take_kick(1), "a kick must be consumed exactly once");
}

#[test]
fn bind_addresses_are_parsed_and_rotated() {
    let args = Opts {
        addrs: vec!["127.0.0.1:1883".into()],
        ifaddrs: vec!["127.0.0.1".into()],
        conns: 3,
        ..Default::default()
    };
    let c = Ctx::for_test(args, Protocol::V3);
    assert_eq!(c.peers.len(), 1);
    assert_eq!(
        c.endpoints(1),
        (c.peers[0], IpAddr::from([127, 0, 0, 1]).into())
    );
}

#[test]
fn ctx_creation_rejects_unresolvable_addresses() {
    let args = Opts {
        addrs: vec!["no-such-host.invalid:1883".into()],
        ..Default::default()
    };
    assert!(Ctx::new(args, Protocol::V3, Arc::new(AtomicBool::new(false))).is_err());

    let args = Opts {
        ifaddrs: vec!["not-an-ip".into()],
        ..Default::default()
    };
    let err = Ctx::new(args, Protocol::V3, Arc::new(AtomicBool::new(false))).unwrap_err();
    assert!(err.to_string().contains("not an IP"), "{err}");
}

#[test]
fn paced_publish_emits_one_message_per_tick() {
    // QoS 1 so the pacing can also be observed in the inflight window.
    let c = Arc::new(Ctx::for_test(
        Opts {
            pub_interval: 100,
            ..c_args(1, 5, 0)
        },
        Protocol::V3,
    ));
    let mut out = Outbound::new(&c, 0, None);
    let mut pending = Vec::new();
    for _ in 0..4 {
        out.fill(&c, "id", &mut pending);
    }
    assert_eq!(pending.len(), 4, "--pub-interval paces 1 msg per tick");
    assert_eq!(out.inflight.len(), 4);
}

#[test]
fn v5_broker_receive_maximum_caps_the_window() {
    let c = ctx(1, 100, 0); // local window 100

    // The broker advertises Receive Maximum 16 -> the window must shrink.
    let mut out = Outbound::new(&c, 0, Some(16));
    let mut pending = Vec::new();
    out.fill(&c, "id", &mut pending);
    assert_eq!(out.inflight.len(), 16, "receive max 16 caps the burst");
    assert_eq!(c.stats.snapshot().inflight, 16);

    // A larger broker value still respects the local --max-inflight.
    let mut small = Outbound::new(&c, 0, Some(500));
    small.fill(&c, "id", &mut pending);
    assert_eq!(
        small.inflight.len(),
        100,
        "local --max-inflight wins when smaller"
    );

    // v3 (no advertisement) keeps the local window untouched.
    let mut v3 = Outbound::new(&c, 0, None);
    v3.fill(&c, "id", &mut pending);
    assert_eq!(v3.inflight.len(), 100);

    // A zero receive max must not produce a zero-sized (dead) window.
    let mut zero = Outbound::new(&c, 0, Some(0));
    assert_eq!(zero.slots.capacity(), 1);
    assert!(zero.slots.take().is_some());
}

#[test]
fn qos0_fill_is_bounded_by_window_and_limit() {
    let c = ctx(0, 5, 7);
    let mut out = Outbound::new(&c, 0, None);
    let mut pending = Vec::new();

    out.fill(&c, "id", &mut pending);
    assert_eq!(pending.len(), 5, "one --max-inflight batch per tick");
    assert_eq!(
        out.inflight.len(),
        0,
        "qos0 must not track inflight messages"
    );
    assert_eq!(c.stats.snapshot().sends, 5);

    pending.clear();
    out.fill(&c, "id", &mut pending);
    assert_eq!(pending.len(), 2, "--max-limit caps the second burst");
    assert!(out.limit_reached(&c));
}

#[test]
fn qos1_fill_blocks_when_the_window_is_full() {
    let c = ctx(1, 2, 0);
    let mut out = Outbound::new(&c, 0, None);
    let mut pending = Vec::new();
    out.fill(&c, "id", &mut pending);

    assert_eq!(pending.len(), 2);
    assert_eq!(out.inflight.len(), 2);
    assert_eq!(c.stats.snapshot().inflight, 2);

    out.fill(&c, "id", &mut pending);
    assert_eq!(
        c.stats.snapshot().inflight_full,
        1,
        "blocked produce tick counted"
    );

    let pid = publish_pid(pending.first().unwrap());
    out.on_ack(&c, pid);
    assert_eq!(c.stats.snapshot().inflight, 1);
    assert_eq!(c.stats.snapshot().pub_acks, 1);

    // Acking an unknown id must not corrupt the window.
    out.on_ack(&c, 999);
    assert_eq!(c.stats.snapshot().inflight, 1);
}

#[test]
fn qos2_handshake_releases_the_slot_on_pubcomp() {
    let c = ctx(2, 4, 0);
    let mut out = Outbound::new(&c, 0, None);
    let mut pending = Vec::new();
    out.fill(&c, "id", &mut pending);

    out.on_pubrec(&c, 1, &mut pending);
    assert!(
        matches!(pending.last(), Some(Packet::PubRel { pid: 1, reason: 2 })),
        "PUBREC must be answered with PUBREL"
    );
    assert_eq!(out.inflight.len(), 4, "still inflight until PUBCOMP");
    assert_eq!(
        out.unreleased_pubrel(),
        1,
        "the release is awaiting PUBCOMP"
    );

    let before = pending.len();
    out.on_pubrec(&c, 999, &mut pending);
    assert_eq!(
        pending.len(),
        before,
        "an unknown packet id must not emit PUBREL"
    );

    out.on_ack(&c, 1);
    assert_eq!(out.inflight.len(), 3);
}

#[test]
fn ack_timeout_frees_the_window() {
    let c = ctx(1, 2, 0);
    let mut out = Outbound::new(&c, 0, None);
    let mut pending = Vec::new();
    out.fill(&c, "id", &mut pending);
    assert_eq!(c.stats.snapshot().inflight, 2);

    out.sweep_timeouts(&c, c.now_us() + 60_000_000); // pretend a minute went by

    let snap = c.stats.snapshot();
    assert_eq!(snap.ack_timeouts, 2);
    assert_eq!(snap.inflight, 0);
    assert!(snap.errors >= 2, "{snap:?}");
    assert!(out.inflight.is_empty());
    assert_eq!(out.slots.in_use(), 0);
}

#[test]
fn disabled_sweep_is_a_noop() {
    let c = ctx(1, 2, 0);
    let mut out = Outbound::new(&c, 0, None);
    let mut pending = Vec::new();
    out.fill(&c, "id", &mut pending);

    // Same "long ago" timestamps as the previous test, but the timeout is off.
    let args = Opts {
        ack_timeout: 0,
        ..c.args.clone()
    };
    let disabled = Arc::new(Ctx::for_test(args, Protocol::V3));
    out.sweep_timeouts(&disabled, u64::MAX / 2);

    assert_eq!(disabled.stats.snapshot().ack_timeouts, 0);
    assert_eq!(
        c.stats.snapshot().inflight,
        2,
        "a disabled sweep must touch nothing"
    );
    assert_eq!(out.inflight.len(), 2);
}

#[test]
fn inbound_publishes_are_counted_and_acknowledged() {
    let c = ctx(1, 4, 0);
    let mut out = Outbound::new(&c, 0, None);
    let mut awaiting = vec![];
    let mut pending = Vec::new();

    let ts = c.now_us();
    let payload = c.payload.build(ts);
    // QoS 1 inbound: PUBACK expected.
    on_incoming(
        &c,
        Incoming::Publish {
            topic: "t/1".into(),
            payload: payload.clone(),
            qos: 1,
            retain: false,
            packet_id: Some(11),
        },
        &mut out,
        &mut awaiting,
        &mut pending,
    )
    .unwrap();
    assert_eq!(c.stats.snapshot().recvs, 1);
    assert_eq!(
        c.stats.latency_stats().samples,
        1,
        "stamped payload must yield a latency sample"
    );
    assert!(matches!(pending[0], Packet::PubAck { pid: 11, .. }));

    // QoS 2 inbound: PUBREC expected, PUBREL completes it.
    pending.clear();
    on_incoming(
        &c,
        Incoming::Publish {
            topic: "t/1".into(),
            payload: payload.clone(),
            qos: 2,
            retain: false,
            packet_id: Some(12),
        },
        &mut out,
        &mut awaiting,
        &mut pending,
    )
    .unwrap();
    assert!(matches!(pending[0], Packet::PubRec { pid: 12, .. }));
    assert_eq!(awaiting, vec![12]);

    pending.clear();
    on_incoming(
        &c,
        Incoming::PubRel { pid: 12 },
        &mut out,
        &mut awaiting,
        &mut pending,
    )
    .unwrap();
    assert!(matches!(pending[0], Packet::PubComp { pid: 12, .. }));
    assert!(awaiting.is_empty());

    // Redelivery of the same pid is reported as a duplicate.
    pending.clear();
    on_incoming(
        &c,
        Incoming::Publish {
            topic: "t/1".into(),
            payload,
            qos: 2,
            retain: false,
            packet_id: Some(12),
        },
        &mut out,
        &mut awaiting,
        &mut pending,
    )
    .unwrap();
    awaiting.push(12);
    on_incoming(
        &c,
        Incoming::Publish {
            topic: "t/1".into(),
            payload: c.payload.build(c.now_us()),
            qos: 2,
            retain: false,
            packet_id: Some(13),
        },
        &mut out,
        &mut awaiting,
        &mut pending,
    )
    .unwrap();
    assert_eq!(c.stats.snapshot().dups, 0);
    on_incoming(
        &c,
        Incoming::Publish {
            topic: "t/1".into(),
            payload: c.payload.build(c.now_us()),
            qos: 2,
            retain: false,
            packet_id: Some(13),
        },
        &mut out,
        &mut awaiting,
        &mut pending,
    )
    .unwrap();
    assert_eq!(c.stats.snapshot().dups, 1);
}

#[test]
fn server_disconnect_is_propagated_as_error() {
    let c = ctx(1, 4, 0);
    let mut out = Outbound::new(&c, 0, None);
    let mut awaiting = vec![];
    let mut pending = Vec::new();
    let err = on_incoming(
        &c,
        Incoming::ServerDisconnect("NotAuthorized".into()),
        &mut out,
        &mut awaiting,
        &mut pending,
    )
    .unwrap_err();
    assert!(err.to_string().contains("NotAuthorized"), "{err}");

    let err = on_incoming(
        &c,
        Incoming::ConnAck {
            accepted: false,
            reason: "BadUserNameOrPassword".into(),
            receive_max: None,
        },
        &mut out,
        &mut awaiting,
        &mut pending,
    )
    .unwrap_err();
    assert!(err.to_string().contains("BadUserNameOrPassword"), "{err}");
}

#[test]
fn packets_not_relevant_for_the_bench_are_ignored() {
    let c = ctx(1, 4, 0);
    let mut out = Outbound::new(&c, 0, None);
    let mut awaiting = vec![];
    let mut pending = Vec::new();
    for pkt in [
        Incoming::Ignored,
        Incoming::PingResp,
        Incoming::ConnAck {
            accepted: true,
            reason: "ok".into(),
            receive_max: None,
        },
        Incoming::SubAck {
            granted: vec![Some(1)],
        },
    ] {
        assert!(on_incoming(&c, pkt, &mut out, &mut awaiting, &mut pending).is_ok());
    }
    assert!(pending.is_empty());
}

#[test]
fn outbound_publish_packets_are_wellformed() {
    let c = ctx(1, 2, 0);
    let mut out = Outbound::new(&c, 0, None);
    let mut pending = Vec::new();
    out.fill(&c, "id-1", &mut pending);
    for pkt in &pending {
        let Packet::Publish(p) = pkt else {
            panic!("expected PUBLISH")
        };
        assert!(p.topic.starts_with("t/"), "{}", p.topic);
        assert_eq!(p.payload.len(), 32);
        assert!(p.pid.is_some(), "qos1 must carry a packet id");
        assert!(
            PayloadBuilder::parse(&p.payload).is_some(),
            "payload must carry the latency stamp"
        );
    }
    let ids: Vec<u16> = pending.iter().map(publish_pid).collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
}

fn publish_pid(pkt: &Packet) -> u16 {
    match pkt {
        Packet::Publish(p) => p.pid.unwrap_or(0),
        other => panic!("expected PUBLISH, got {other:?}"),
    }
}

#[tokio::test]
async fn wait_stopped_returns_when_the_flag_is_set() {
    let flag = Arc::new(AtomicBool::new(false));
    let bg = flag.clone();
    tokio::spawn(async move {
        time::sleep(Duration::from_millis(20)).await;
        bg.store(true, Ordering::Relaxed);
    });
    time::timeout(Duration::from_secs(3), wait_stopped(flag))
        .await
        .unwrap();
}

#[tokio::test]
async fn wait_stopped_returns_immediately_when_already_set() {
    let flag = Arc::new(AtomicBool::new(true));
    time::timeout(Duration::from_millis(200), wait_stopped(flag))
        .await
        .unwrap();
}
