//! Packet construction and the protocol-independent view of inbound packets.
//!
//! Everything here is written against [`crate::mqtt`], the benchmark's own
//! MQTT implementation — the broker's codec is never involved, so a framing
//! defect on either side shows up as a failed connection instead of being
//! silently agreed with.

use bytes::Bytes;

use crate::client::Ctx;
use crate::mqtt::{self, Connect, Packet, Publish, QoS, SubOpts, Will};
use crate::options::{Opts, Protocol};

/// Wire size of a PUBLISH packet, used for throughput accounting.
pub fn publish_wire_size(proto: Protocol, topic: &str, payload_len: usize, qos: u8) -> usize {
    mqtt::publish_wire_size(proto.version(), topic, payload_len, qos)
}

/// CONNECT with the credentials, keepalive and will from the options.
pub fn connect(ctx: &Ctx, client_id: &str) -> Packet {
    let args = &ctx.args;
    let mut conn = Connect::new(client_id, args.keepalive, args.clean);
    conn.username = args.username.clone();
    conn.password = args.password.as_ref().map(|p| p.as_bytes().to_vec());
    conn.will = will(args);
    if ctx.proto == Protocol::V5 {
        // Keep a non-clean session alive for two hours, matching the broker
        // default so that session state can be measured.
        conn.session_expiry = if args.clean { None } else { Some(7200) };
    }
    Packet::Connect(conn)
}

fn will(args: &Opts) -> Option<Will> {
    let topic = args.lw_topic.as_ref()?;
    Some(Will {
        topic: topic.clone(),
        payload: args.lw_msg.as_deref().unwrap_or("").as_bytes().to_vec(),
        qos: QoS::from_u8(args.lw_qos),
        retain: args.lw_retain,
    })
}

/// SUBSCRIBE for a single topic filter.
pub fn subscribe(ctx: &Ctx, filter: &str, pid: u16) -> Packet {
    Packet::Subscribe {
        pid,
        filters: vec![(filter.to_owned(), SubOpts { qos: ctx.qos() })],
    }
}

/// PUBLISH with an already built payload.
pub fn publish(ctx: &Ctx, topic: &str, payload: Bytes, pid: Option<u16>) -> Packet {
    Packet::Publish(Publish {
        topic: topic.to_owned(),
        payload,
        qos: ctx.qos(),
        retain: ctx.args.retain,
        dup: false,
        pid,
    })
}

/// The four acknowledgement packets of the MQTT delivery handshakes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(
    clippy::enum_variant_names,
    reason = "names mirror the MQTT packet types"
)]
pub enum Ack {
    /// QoS 1 completion
    PubAck,
    /// QoS 2 step 2 (receiver -> sender)
    PubRec,
    /// QoS 2 step 3 (sender -> receiver)
    PubRel,
    /// QoS 2 step 4 (receiver -> sender)
    PubComp,
}

/// Build an acknowledgement packet.
///
/// Only v5 carries reason codes; for PUBREL the value is configurable because
/// MQTT 5.0 mandates `0x02` (Send Onward) while oximqttd <= 0.22 closes the
/// connection on it (see `--v5-pubrel-reason`).
pub fn ack(ctx: &Ctx, kind: Ack, pid: u16) -> Packet {
    match kind {
        Ack::PubAck => Packet::PubAck { pid, reason: 0 },
        Ack::PubRec => Packet::PubRec { pid, reason: 0 },
        Ack::PubRel => Packet::PubRel {
            pid,
            reason: ctx.args.v5_pubrel_reason,
        },
        Ack::PubComp => Packet::PubComp { pid, reason: 0 },
    }
}

/// Reply an inbound receiver must send for a publish of `qos`.
pub fn reply_for(qos: u8, packet_id: Option<u16>) -> Option<(Ack, u16)> {
    match (qos, packet_id) {
        (1, Some(pid)) => Some((Ack::PubAck, pid)),
        (2, Some(pid)) => Some((Ack::PubRec, pid)),
        _ => None,
    }
}

/// PINGREQ
pub fn ping_req() -> Packet {
    Packet::PingReq
}

/// DISCONNECT
pub fn disconnect() -> Packet {
    Packet::Disconnect { reason: 0 }
}

/// Protocol independent view of an inbound packet.
#[derive(Debug)]
pub enum Incoming {
    /// CONNACK received; `reason` names the code, `receive_max` the broker's
    /// advertised v5 Receive Maximum (None for v3 / absent).
    ConnAck {
        accepted: bool,
        reason: String,
        receive_max: Option<u16>,
    },
    /// SUBACK received; one entry per filter, `None` when refused.
    SubAck { granted: Vec<Option<u8>> },
    /// PUBLISH received
    Publish {
        topic: String,
        payload: Bytes,
        qos: u8,
        retain: bool,
        packet_id: Option<u16>,
    },
    /// PUBACK (QoS 1 completion of our publish)
    PubAck { pid: u16 },
    /// PUBREC (QoS 2 step 2 for our publish)
    PubRec { pid: u16 },
    /// PUBREL (QoS 2 step 3 for an inbound publish)
    PubRel { pid: u16 },
    /// PUBCOMP (QoS 2 step 4 for our publish)
    PubComp { pid: u16 },
    /// PINGRESP
    PingResp,
    /// Server initiated DISCONNECT
    ServerDisconnect(String),
    /// Anything the benchmark does not care about
    Ignored,
}

/// Normalize a wire packet into [`Incoming`].
pub fn classify(pkt: &Packet) -> Incoming {
    match pkt {
        Packet::ConnAck {
            reason,
            receive_max,
            ..
        } => Incoming::ConnAck {
            accepted: *reason == 0,
            reason: connack_name(*reason).into(),
            receive_max: *receive_max,
        },
        Packet::SubAck { reasons, .. } => Incoming::SubAck {
            granted: reasons
                .iter()
                .map(|r| if *r < 0x80 { Some(*r) } else { None })
                .collect(),
        },
        Packet::Publish(p) => Incoming::Publish {
            topic: p.topic.clone(),
            payload: p.payload.clone(),
            qos: p.qos.as_u8(),
            retain: p.retain,
            packet_id: p.pid,
        },
        Packet::PubAck { pid, .. } => Incoming::PubAck { pid: *pid },
        Packet::PubRec { pid, .. } => Incoming::PubRec { pid: *pid },
        Packet::PubRel { pid, .. } => Incoming::PubRel { pid: *pid },
        Packet::PubComp { pid, .. } => Incoming::PubComp { pid: *pid },
        Packet::PingResp => Incoming::PingResp,
        Packet::Disconnect { reason } => Incoming::ServerDisconnect(connack_name(*reason).into()),
        Packet::UnsubAck { .. }
        | Packet::Connect(_)
        | Packet::Subscribe { .. }
        | Packet::Unsubscribe { .. }
        | Packet::PingReq => Incoming::Ignored,
    }
}

/// Human readable name of a CONNACK / SUBACK / DISCONNECT reason code.
fn connack_name(code: u8) -> &'static str {
    match code {
        0 => "Accepted",
        1 => "UnacceptableProtocolVersion",
        2 => "IdentifierRejected",
        3 => "ServerUnavailable",
        4 => "BadUserNameOrPassword",
        5 => "NotAuthorized",
        6..=0x7F => "ReservedSuccess",
        0x80 => "UnspecifiedError",
        0x81 => "MalformedPacket",
        0x82 => "ProtocolError",
        0x83 => "ImplementationSpecificError",
        0x84 => "UnsupportedProtocolVersion",
        0x85 => "ClientIdentifierNotValid",
        0x86 => "BadAuthenticationMethod",
        0x87 => "NotAuthorized",
        0x88 => "ServerUnavailable",
        0x89 => "ServerBusy",
        0x8A => "Banned",
        0x8C => "BadUserNameOrPassword",
        0x90 => "TopicNameInvalid",
        0x95 => "QuotaExceeded",
        0x97 => "PayloadFormatInvalid",
        0x99 => "TopicFilterInvalid",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mqtt::Version;
    use crate::options::Opts;

    /// v3.1.1 has no reason codes, so acks come back with reason 0.
    fn normalize(proto: Protocol, pkt: &Packet) -> Packet {
        if proto == Protocol::V3 {
            return match pkt {
                Packet::PubRel { pid, .. } => Packet::PubRel {
                    pid: *pid,
                    reason: 0,
                },
                other => other.clone(),
            };
        }
        pkt.clone()
    }

    fn ctx(proto: Protocol) -> Ctx {
        ctx_with_qos(proto, 2)
    }

    #[allow(clippy::needless_pass_by_value)]
    fn ctx_with_qos(proto: Protocol, qos: u8) -> Ctx {
        let args = Opts {
            addrs: vec!["127.0.0.1:1883".into()],
            username: Some("u".into()),
            password: Some("p".into()),
            sub: true,
            pub_switch: true,
            qos,
            topic_no_range: vec![0, 9],
            max_inflight: 10,
            ..Default::default()
        };
        Ctx::for_test(args, proto)
    }

    #[test]
    fn every_outgoing_packet_encodes_and_decodes() {
        for proto in [Protocol::V3, Protocol::V5] {
            let ctx = ctx(proto);
            #[allow(unused_mut)]
            let mut pkts = vec![
                subscribe(&ctx, "t/1", 3),
                publish(&ctx, "t/1", Bytes::from_static(b"payload"), Some(3)),
                ack(&ctx, Ack::PubAck, 3),
                ack(&ctx, Ack::PubRec, 3),
                ack(&ctx, Ack::PubRel, 3),
                ack(&ctx, Ack::PubComp, 3),
                ping_req(),
                disconnect(),
            ];
            pkts.insert(0, connect(&ctx, "bench-1"));
            // A QoS 0 publisher sends PUBLISH without a packet id.
            let qos0 = ctx_with_qos(proto, 0);
            pkts.push(publish(&qos0, "t/1", Bytes::from_static(b"x"), None));
            pkts.push(subscribe(&qos0, "t/#", 9));

            for p in &pkts {
                let buf = mqtt::encode_packet(proto.version(), p)
                    .unwrap_or_else(|e| panic!("{proto:?} {p:?}: {e}"));
                let mut buf: bytes::BytesMut = buf;
                let back = mqtt::decode_all(proto.version(), &mut buf)
                    .unwrap()
                    .pop()
                    .expect("one packet decoded");
                assert_eq!(&back, &normalize(proto, p), "{proto:?} roundtrip");
                assert!(buf.is_empty());
            }
        }
    }

    #[test]
    fn connect_packet_carries_credentials_and_will() {
        let base = ctx(Protocol::V3);
        let Packet::Connect(c) = connect(&base, "abc") else {
            panic!("expected CONNECT")
        };
        assert_eq!(c.client_id, "abc");
        assert_eq!(c.username.as_deref(), Some("u"));
        assert_eq!(c.password.as_deref(), Some(&b"p"[..]));
        assert!(c.clean);
        assert_eq!(c.keepalive, 60);
        assert!(c.will.is_none());
        assert_eq!(
            mqtt::encode_packet(Version::V3, &Packet::Connect(c.clone())).unwrap()[2..6],
            [0, 4, b'M', b'Q']
        );

        let mut with_will = ctx(Protocol::V3);
        with_will.args.lw_topic = Some("will/topic".into());
        with_will.args.lw_msg = Some("bye".into());
        with_will.args.lw_qos = 1;
        with_will.args.lw_retain = true;
        let Packet::Connect(c) = connect(&with_will, "abc") else {
            panic!("expected CONNECT")
        };
        let w = c.will.clone().unwrap();
        assert_eq!(w.topic, "will/topic");
        assert_eq!(w.payload, b"bye".to_vec());
        assert_eq!(w.qos, QoS::AtLeastOnce);
        assert!(w.retain);

        // v5 adds the session expiry property when the session is kept alive.
        let mut kept = ctx(Protocol::V5);
        kept.args.clean = false;
        let Packet::Connect(c) = connect(&kept, "abc") else {
            panic!("expected CONNECT")
        };
        assert_eq!(c.session_expiry, Some(7200));
        let Packet::Connect(c) = connect(&base, "abc") else {
            panic!("expected CONNECT")
        };
        assert_eq!(
            c.session_expiry, None,
            "v3.1.1 has no session expiry property"
        );
    }

    #[test]
    fn classify_normalizes_inbound_packets() {
        type Check = fn(&Incoming) -> bool;
        let cases: Vec<(Packet, Check)> = vec![
            (
                Packet::ConnAck {
                    reason: 0,
                    session_present: false,
                    receive_max: None,
                },
                |i| matches!(i, Incoming::ConnAck { accepted: true, .. }),
            ),
            (
                Packet::ConnAck {
                    reason: 5,
                    session_present: false,
                    receive_max: Some(16),
                },
                |i| {
                    matches!(i, Incoming::ConnAck { accepted: false, reason, receive_max: Some(16) }
                        if reason == "NotAuthorized")
                },
            ),
            (
                Packet::SubAck {
                    pid: 1,
                    reasons: vec![0, 2],
                },
                |i| matches!(i, Incoming::SubAck { granted } if granted == &vec![Some(0), Some(2)]),
            ),
            (
                Packet::SubAck {
                    pid: 1,
                    reasons: vec![0x87],
                },
                |i| matches!(i, Incoming::SubAck { granted } if granted == &vec![None]),
            ),
            (Packet::PubAck { pid: 5, reason: 0 }, |i| {
                matches!(i, Incoming::PubAck { pid: 5 })
            }),
            (Packet::PubRec { pid: 5, reason: 0 }, |i| {
                matches!(i, Incoming::PubRec { pid: 5 })
            }),
            (Packet::PubRel { pid: 5, reason: 2 }, |i| {
                matches!(i, Incoming::PubRel { pid: 5 })
            }),
            (Packet::PubComp { pid: 5, reason: 0 }, |i| {
                matches!(i, Incoming::PubComp { pid: 5 })
            }),
            (Packet::PingResp, |i| matches!(i, Incoming::PingResp)),
            (Packet::Disconnect { reason: 0 }, |i| {
                matches!(i, Incoming::ServerDisconnect(_))
            }),
            (
                Packet::UnsubAck {
                    pid: 1,
                    reasons: vec![],
                },
                |i| matches!(i, Incoming::Ignored),
            ),
            (
                Packet::Publish(Publish {
                    topic: "a/b".into(),
                    payload: Bytes::from_static(b"0123456789"),
                    qos: QoS::AtLeastOnce,
                    retain: true,
                    dup: false,
                    pid: Some(7),
                }),
                |i| matches!(i, Incoming::Publish { topic, qos: 1, retain: true, packet_id: Some(7), payload } if topic == "a/b" && payload.len() == 10),
            ),
        ];
        for (pkt, check) in cases {
            let got = classify(&pkt);
            assert!(check(&got), "classify produced the wrong result: {got:?}");
        }
    }

    #[test]
    fn reply_for_matches_qos() {
        assert_eq!(reply_for(0, Some(1)), None);
        assert_eq!(reply_for(1, None), None);
        assert_eq!(reply_for(1, Some(2)), Some((Ack::PubAck, 2)));
        assert_eq!(reply_for(2, Some(3)), Some((Ack::PubRec, 3)));
    }

    #[test]
    fn ack_reasons_follow_the_spec() {
        // PUBREL must carry reason 2 on the wire (checked byte-exactly in the
        // codec tests); the receiver-side acks are plain success.
        let c = ctx(Protocol::V5);
        assert_eq!(
            ack(&c, Ack::PubRel, 1),
            Packet::PubRel { pid: 1, reason: 2 }
        );
        assert_eq!(
            ack(&c, Ack::PubAck, 1),
            Packet::PubAck { pid: 1, reason: 0 }
        );
        let mut compat = ctx(Protocol::V5);
        compat.args.v5_pubrel_reason = 0;
        assert_eq!(
            ack(&compat, Ack::PubRel, 1),
            Packet::PubRel { pid: 1, reason: 0 }
        );
    }

    #[test]
    fn wire_size_matches_the_encoder() {
        for proto in [Protocol::V3, Protocol::V5] {
            for qos in 0..3u8 {
                let c = ctx_with_qos(proto, qos);
                let p = publish(
                    &c,
                    "a/b",
                    Bytes::from(vec![0u8; 33]),
                    (qos > 0).then_some(4),
                );
                let len = mqtt::encode_packet(proto.version(), &p).unwrap().len();
                assert_eq!(
                    len,
                    publish_wire_size(proto, "a/b", 33, qos),
                    "{proto:?} qos{qos}"
                );
            }
        }
    }

    #[test]
    fn unknown_reason_codes_are_named() {
        assert_eq!(connack_name(0x8A), "Banned");
        assert_eq!(connack_name(250), "Unknown");
    }
}
