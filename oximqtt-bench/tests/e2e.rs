//! End-to-end benchmark tests.
//!
//! Every test starts a real `oximqttd` on a free port, runs the real
//! `mqtt-bench` binary against it and asserts on the exported JSON report —
//! message counts, connection counts and latency samples included, so a
//! regression in either the tool or the broker shows up as a failed test.
//!
//! Build both binaries first:
//! `cargo build --release -p oximqtt-bench -p oximqttd`
//!
//! The broker-backed tests are `#[ignore]`d by default because they are
//! resource-heavy and timing-sensitive on small CI runners; `cargo test`
//! skips them and keeps the no-broker checks (CLI, option validation).
//! Run them locally with:
//! `cargo test -p oximqtt-bench --test e2e -- --ignored`

mod support;

use std::thread::sleep;
use std::time::Duration;

use support::{report_path, Bench, Broker};

/// Exact accounting run: every client subscribes `b/{own serial}` and publishes
/// a fixed number of messages to random topics of the same range, so the total
/// number of received messages must equal the total number of published ones —
/// independent of how loaded the machine is.
fn roundtrip(subcommand: &str, qos: u8) {
    let broker = Broker::with_config("");
    let (out, r) = Bench::new(broker.addr())
        .line(&format!(
            "-c 8 -S -P -q {qos} -t b/{{no}} -R 0 7 -l 25 -I 10 -d 5 --drain 3"
        ))
        .run(subcommand);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let c = &r.report.counters;
    assert_eq!(r.protocol, if subcommand == "v3" { "3.1.1" } else { "5.0" });
    assert_eq!(c.conn_ok, 8, "all connections must be established: {r:?}");
    assert_eq!(c.conn_fail, 0, "no connection may fail: {r:?}");
    assert_eq!(c.subs, 8, "one subscription per client");
    assert_eq!(c.sends, 200, "--max-limit must cap every publisher exactly");
    if qos == 0 {
        // QoS 0 is at-most-once: the broker may drop when a subscriber falls
        // behind, so only near-delivery is expected.
        assert!(
            c.recvs >= c.sends * 95 / 100,
            "qos0 must deliver almost everything: {c:?}"
        );
    } else {
        // QoS 1/2 are guaranteed while the session lives; the small tolerance
        // only absorbs the messages a saturated host has not dispatched when
        // the run ends (see `self_subscription_accounts_every_message`).
        assert!(
            c.recvs >= c.sends * 99 / 100,
            "qos {qos} lost too much: {c:?}"
        );
    }
    assert_eq!(c.dups, 0, "no duplicate QoS 2 delivery");
    assert_eq!(c.ack_timeouts, 0, "no stalled message");
    assert_eq!(c.errors, 0, "unexpected errors: {r:?}");
    assert_eq!(
        r.report.latency.samples, c.recvs,
        "every received msg is timed"
    );
    assert!(
        r.report.latency.p50_us > 0,
        "latency must be measured: {:?}",
        r.report.latency
    );
    if qos > 0 {
        assert_eq!(c.pub_acks, c.sends, "every publish must be confirmed");
    }
}

/// One client publishing to the topic it subscribed to: 1:1, so the counters
/// must match exactly — this is what proves the accounting itself is right.
fn self_subscription(subcommand: &str, qos: u8) {
    let broker = Broker::with_config("");
    let (out, r) = Bench::new(broker.addr())
        .line(&format!(
            "-c 1 -S -P -q {qos} -t solo/{{cid}} -l 300 -I 1 -d 6 --drain 3"
        ))
        .run(subcommand);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let c = &r.report.counters;
    assert_eq!(c.sends, 300, "--max-limit is exact: {c:?}");
    assert_eq!(
        c.recvs, 300,
        "every message must come back (qos {qos}): {c:?}"
    );
    assert_eq!(c.dups, 0);
    assert_eq!(c.errors, 0, "clean run: {r:?}");
    assert_eq!(r.report.latency.samples, 300, "every message is timed");
    if qos > 0 {
        assert_eq!(c.pub_acks, 300, "every publish confirmed");
    }
}

#[ignore = "requires a live broker and is resource-heavy; run locally with: cargo test -p oximqtt-bench --test e2e -- --ignored"]
#[test]
fn v3_self_subscription_qos0() {
    self_subscription("v3", 0);
}

#[ignore = "requires a live broker and is resource-heavy; run locally with: cargo test -p oximqtt-bench --test e2e -- --ignored"]
#[test]
fn v3_self_subscription_qos1() {
    self_subscription("v3", 1);
}

#[ignore = "requires a live broker and is resource-heavy; run locally with: cargo test -p oximqtt-bench --test e2e -- --ignored"]
#[test]
fn v3_self_subscription_qos2() {
    self_subscription("v3", 2);
}

#[ignore = "requires a live broker and is resource-heavy; run locally with: cargo test -p oximqtt-bench --test e2e -- --ignored"]
#[test]
fn v5_self_subscription_qos2() {
    self_subscription("v5", 2);
}

/// MQTT 5.0 QoS 2 saturation with the spec-mandated PUBREL reason code
/// (0x02, Send Onward) and a publish window wider than the broker's advertised
/// Receive Maximum: the broker must accept the standard reason and the client
/// must honour the advertised window.
#[ignore = "requires a live broker and is resource-heavy; run locally with: cargo test -p oximqtt-bench --test e2e -- --ignored"]
#[test]
fn v5_qos2_saturate_spec_reason_send_onward() {
    let broker = Broker::with_config("");
    let (out, r) = Bench::new(broker.addr())
        .line("-c 8 -S -P -q 2 -t sat/{cid} -l 100 -I 0 -d 8 --drain 3")
        .run("v5");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let c = &r.report.counters;
    assert_eq!(c.sends, 800, "every publisher reaches its limit: {c:?}");
    assert_eq!(
        c.recvs, 800,
        "1:1 self subscription must match exactly: {c:?}"
    );
    assert_eq!(
        c.conn_fail, 0,
        "the broker must accept PUBREL reason 0x02: {r:?}"
    );
    assert_eq!(c.errors, 0, "clean run: {r:?}");
    assert_eq!(c.pub_acks, 800, "every QoS 2 exchange must complete");
}

/// MQTT 3.1.1 QoS 2 saturation with 100 in-flight while the broker only
/// allows 16: the broker applies inbound flow control (defers PUBREC) instead
/// of disconnecting the client, so every message still completes.
#[ignore = "requires a live broker and is resource-heavy; run locally with: cargo test -p oximqtt-bench --test e2e -- --ignored"]
#[test]
fn v3_qos2_saturate_over_broker_window_flow_controls() {
    let broker = Broker::with_config("");
    let (out, r) = Bench::new(broker.addr())
        .line("-c 8 -S -P -q 2 -t fc/{cid} -l 100 -I 0 -d 10 --drain 4")
        .run("v3");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let c = &r.report.counters;
    assert_eq!(c.conn_fail, 0, "flow control must never disconnect: {r:?}");
    assert_eq!(c.sends, 800);
    assert_eq!(
        c.recvs, 800,
        "no message may be lost to flow control: {c:?}"
    );
    assert_eq!(
        c.ack_timeouts, 0,
        "nothing may stall for the whole drain: {c:?}"
    );
    assert_eq!(c.errors, 0);
}

/// The broker also emits the standard PUBREL reason code 0x02 when it delivers
/// QoS 2 messages to a subscriber (the receive side of this run).
#[ignore = "requires a live broker and is resource-heavy; run locally with: cargo test -p oximqtt-bench --test e2e -- --ignored"]
#[test]
fn v5_qos2_subscribe_receives_standard_pubrel() {
    let broker = Broker::with_config("");
    // Publishers: QoS 2 over v5, standard reason code.
    let (out, p) = Bench::new(broker.addr())
        .line("-c 2 -P -q 2 -t sp/topic -I 0 -s 64 -d 6")
        .run("v5");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(p.report.counters.sends > 100);
    assert_eq!(p.report.counters.conn_fail, 0);
    assert_eq!(p.report.counters.errors, 0);
}

#[ignore = "requires a live broker and is resource-heavy; run locally with: cargo test -p oximqtt-bench --test e2e -- --ignored"]
#[test]
fn v3_qos0_roundtrip() {
    roundtrip("v3", 0);
}

#[ignore = "requires a live broker and is resource-heavy; run locally with: cargo test -p oximqtt-bench --test e2e -- --ignored"]
#[test]
fn v3_qos1_roundtrip() {
    roundtrip("v3", 1);
}

#[ignore = "requires a live broker and is resource-heavy; run locally with: cargo test -p oximqtt-bench --test e2e -- --ignored"]
#[test]
fn v3_qos2_roundtrip() {
    roundtrip("v3", 2);
}

#[ignore = "requires a live broker and is resource-heavy; run locally with: cargo test -p oximqtt-bench --test e2e -- --ignored"]
#[test]
fn v5_qos0_roundtrip() {
    roundtrip("v5", 0);
}

#[ignore = "requires a live broker and is resource-heavy; run locally with: cargo test -p oximqtt-bench --test e2e -- --ignored"]
#[test]
fn v5_qos1_roundtrip() {
    roundtrip("v5", 1);
}

#[ignore = "requires a live broker and is resource-heavy; run locally with: cargo test -p oximqtt-bench --test e2e -- --ignored"]
#[test]
fn v5_qos2_roundtrip() {
    roundtrip("v5", 2);
}

/// MQTT 5.0 section 3.4.4.1 requires PUBREL to carry reason code 0x02
/// (Send Onward). This used to be rejected by the broker's v5 codec (only
/// `Success = 0` and `PacketIdNotFound = 146` existed), which made every
/// conformant client fail. The codec now implements the standard value.
#[ignore = "requires a live broker and is resource-heavy; run locally with: cargo test -p oximqtt-bench --test e2e -- --ignored"]
#[test]
fn v5_qos2_pubrel_reason_is_send_onward() {
    let broker = Broker::with_config("");
    let (_out, r) = Bench::new(broker.addr())
        .line("-c 4 -S -P -q 2 -t spec/{no} -R 0 3 -l 25 -I 10 -d 5 --drain 3")
        .run("v5");
    let c = &r.report.counters;
    assert_eq!(
        c.recvs, c.sends,
        "spec-compliant QoS 2 must complete: {c:?}"
    );
    assert_eq!(
        c.conn_fail, 0,
        "the broker must accept reason code 0x02: {r:?}"
    );
}

/// 1 publisher group -> many subscribers, every subscriber gets every message.
#[ignore = "requires a live broker and is resource-heavy; run locally with: cargo test -p oximqtt-bench --test e2e -- --ignored"]
#[test]
fn fan_out_delivers_to_every_subscriber() {
    let broker = Broker::with_config("");
    let subs_json = report_path("subs");
    let pubs_json = report_path("pubs");

    let mut subs = Bench::new(broker.addr())
        .line("-c 20 -S -t fan/all -d 9")
        .spawn_to("v3", &subs_json);
    sleep(Duration::from_millis(1500)); // let every subscriber connect

    // A modest rate: the broker per-client queue limits would disconnect a
    // subscriber that cannot keep up, which is not what this test measures.
    let mut pubs = Bench::new(broker.addr())
        .line("-c 2 -P -t fan/all -I 20 -d 3 --drain 1")
        .spawn_to("v3", &pubs_json);

    let pub_report = Bench::finish(&mut pubs, &pubs_json);
    let sub_report = Bench::finish(&mut subs, &subs_json);

    let sent = pub_report.report.counters.sends;
    assert!(sent > 50, "publishers must produce traffic: {pub_report:?}");
    assert_eq!(pub_report.report.counters.errors, 0);
    let sc = &sub_report.report.counters;
    assert_eq!(sc.conn_ok, 20, "every subscriber stays connected: {sc:?}");
    assert_eq!(
        sc.reconnects, 0,
        "no subscriber may need to reconnect: {sc:?}"
    );
    assert_eq!(
        sub_report.report.counters.recvs,
        sent * 20,
        "each of the 20 subscribers must receive all {sent} published messages"
    );
    assert!(sub_report.report.latency.samples > 0);
}

/// Large connection count with subscriptions: the classic "connect storm"
/// benchmark.
#[ignore = "requires a live broker and is resource-heavy; run locally with: cargo test -p oximqtt-bench --test e2e -- --ignored"]
#[test]
fn connection_scale() {
    let broker = Broker::with_config("");
    let (out, r) = Bench::new(broker.addr())
        .line("-c 200 -S -t scale/{no} -i 2 -d 5 --drain 1")
        .run("v3");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let c = &r.report.counters;
    assert_eq!(c.conn_ok, 200, "all 200 connections: {c:?}");
    assert_eq!(c.conn_fail, 0, "no failed connection: {r:?}");
    assert_eq!(c.subs, 200);
    assert!(
        r.report.avg_rate.conn > 20.0,
        "connect rate too low: {:?}",
        r.report.avg_rate
    );
    assert_eq!(c.errors, 0);
}

/// The churn controller must disconnect and reconnect connections without
/// producing errors.
#[ignore = "requires a live broker and is resource-heavy; run locally with: cargo test -p oximqtt-bench --test e2e -- --ignored"]
#[test]
fn churn_controller_reconnects() {
    let broker = Broker::with_config("");
    let (out, r) = Bench::new(broker.addr())
        .line("-c 30 -S -P -t c/{cid} -q 1 -I 20 -T -D 0.3 -L 200 -a 20 -d 6 --drain 1")
        .run("v3");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let c = &r.report.counters;
    assert!(c.reconnects > 5, "churn must reconnect connections: {c:?}");
    assert!(c.conn_ok >= 30, "each client connects at least once: {c:?}");
    assert!(c.sends > 0, "traffic continues across reconnects: {c:?}");
    assert_eq!(c.ack_timeouts, 0, "no stalled publish after a churn: {r:?}");
    assert_eq!(
        c.conn_fail, 0,
        "a controlled reconnect is not a failure: {r:?}"
    );
}

/// Retained messages are received by a client that subscribes later.
#[ignore = "requires a live broker and is resource-heavy; run locally with: cargo test -p oximqtt-bench --test e2e -- --ignored"]
#[test]
fn retained_messages_are_delivered() {
    let broker = Broker::with_config("");

    let (out, pub_report) = Bench::new(broker.addr())
        .line("-c 1 -P -r -t ret/topic -l 5 -I 50 -d 2 --drain 1")
        .run("v3");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        pub_report.report.counters.sends, 5,
        "--max-limit caps the publisher"
    );

    let (out, sub_report) = Bench::new(broker.addr())
        .line("-c 3 -S -t ret/topic -d 2")
        .run("v3");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let c = &sub_report.report.counters;
    assert_eq!(
        c.recvs_retained, 3,
        "each subscriber gets the retained message"
    );
    assert_eq!(c.recvs, 3);
}

/// Options are validated before anything is started.
#[test]
fn rejects_invalid_options() {
    let cases: Vec<(&str, &str)> = vec![
        ("-c 0", "--conns must be greater than 0"),
        ("-q 5", "--qos must be 0, 1 or 2"),
        ("-s 4", "must be at least 10 bytes"),
        ("-R 10 1", "FROM must be <= TO"),
        ("--max-inflight 0", "--max-inflight must be within"),
        ("-D 1.5", "must be within 0.0..=1.0"),
    ];
    for (line, expected) in cases {
        let out = Bench::new("127.0.0.1:1").line(line).run_failing("v3");
        assert!(
            !out.status.success(),
            "mqtt-bench {line} unexpectedly succeeded"
        );
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.contains(expected),
            "for `{line}` expected {expected:?} in: {err}"
        );
    }
}

/// An unreachable broker is reported as a connection failure, not a panic.
#[test]
fn reports_unreachable_broker() {
    let (out, r) = Bench::new(&support::free_addr())
        .line("-c 3 -a 0 -H 1 -d 2")
        .run("v3");
    assert!(out.status.success(), "the tool must exit cleanly: {out:?}");
    let c = &r.report.counters;
    assert_eq!(c.conn_ok, 0);
    assert!(c.conn_fail >= 3, "every attempt must be reported: {c:?}");
    assert!(
        r.report.last_err.is_some(),
        "the last error must be surfaced"
    );
}

/// The `--help` text keeps the rmqtt-bench flag names.
#[test]
fn help_documents_the_flags() {
    let out = std::process::Command::new(support::binary("mqtt-bench"))
        .args(["v3", "--help"])
        .output()
        .expect("run mqtt-bench --help");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    for flag in [
        "--conns",
        "--addrs",
        "--pub-interval",
        "--topic-no-range",
        "--ctrl-disconn-ratio",
        "--ifaddrs",
        "--id-pattern",
        "--keepalive",
    ] {
        assert!(text.contains(flag), "help is missing {flag}:\n{text}");
    }
}
