//! MQTT packet encoder.

use bytes::{BufMut, BytesMut};

use crate::mqtt::types::*;

/// Fixed header flags of a PUBLISH packet.
fn publish_flags(p: &Publish) -> u8 {
    0x30 | ((p.dup as u8) << 3) | ((p.qos.as_u8()) << 1) | (p.retain as u8)
}

/// Number of bytes the remaining-length varint occupies.
pub fn remaining_length_len(len: usize) -> usize {
    match len {
        0..=127 => 1,
        128..=16_383 => 2,
        16_384..=2_097_151 => 3,
        _ => 4,
    }
}

/// Append a variable length integer (MQTT 1.5.5).
pub fn put_remaining_length(dst: &mut BytesMut, mut len: usize) {
    loop {
        let mut byte = (len & 0x7F) as u8;
        len >>= 7;
        if len > 0 {
            byte |= 0x80;
        }
        dst.put_u8(byte);
        if len == 0 {
            break;
        }
    }
}

/// Length of a UTF-8 string including its 2-byte prefix.
pub const fn string_len(s: &str) -> usize {
    2 + s.len()
}

/// Write a length-prefixed UTF-8 string.
pub fn put_string(dst: &mut BytesMut, s: &str) {
    dst.put_u16(s.len() as u16);
    dst.put_slice(s.as_bytes());
}

/// Write a length-prefixed binary block.
pub fn put_binary(dst: &mut BytesMut, b: &[u8]) {
    dst.put_u16(b.len() as u16);
    dst.put_slice(b);
}

/// Encoded size of the v5 property block written by [`put_v5_properties`].
const EMPTY_PROPS: u8 = 0;

/// Encoded size of a `4B4` v5 property.
const fn prop_len_u32(value: Option<u32>) -> usize {
    match value {
        Some(_) => 5,
        None => 0,
    }
}

/// Wire size of a PUBLISH packet for a given protocol version.
pub fn publish_wire_size(version: Version, topic: &str, payload_len: usize, qos: u8) -> usize {
    let body = string_len(topic) + usize::from(qos > 0) * 2 + payload_len;
    let props = usize::from(version == Version::V5);
    1 + remaining_length_len(body + props) + body + props
}

/// Encode `pkt` for `version` into `dst`.
pub fn encode(version: Version, pkt: &Packet, dst: &mut BytesMut) -> Result<()> {
    let mut body = BytesMut::with_capacity(64);
    let flags = match pkt {
        Packet::Connect(c) => {
            encode_connect(version, c, &mut body)?;
            0x10
        }
        Packet::ConnAck { .. } => return Err(CodecError::Unsupported("sending CONNACK")),
        Packet::Publish(p) => {
            encode_publish(version, p, &mut body)?;
            publish_flags(p)
        }
        Packet::PubAck { pid, reason } => {
            encode_ack(version, *pid, *reason, &mut body)?;
            0x40
        }
        Packet::PubRec { pid, reason } => {
            encode_ack(version, *pid, *reason, &mut body)?;
            0x50
        }
        Packet::PubRel { pid, reason } => {
            // Reserved bits of PUBREL must be 0010. The reason code is passed
            // through as given: MQTT 5.0 mandates 0x02 (Send Onward), but the
            // caller can ask for 0 (see --v5-pubrel-reason) to work around
            // brokers that reject the standard value.
            encode_ack(version, *pid, *reason, &mut body)?;
            0x62
        }
        Packet::PubComp { pid, reason } => {
            encode_ack(version, *pid, *reason, &mut body)?;
            0x70
        }
        Packet::Subscribe { pid, filters } => {
            body.put_u16(*pid);
            if version == Version::V5 {
                body.put_u8(EMPTY_PROPS);
            }
            for (filter, opts) in filters {
                put_string(&mut body, filter);
                body.put_u8(opts.qos.as_u8());
            }
            0x82
        }
        Packet::SubAck { pid, reasons } => suback_body(version, *pid, reasons, &mut body)?,
        Packet::Unsubscribe { pid, filters } => {
            body.put_u16(*pid);
            if version == Version::V5 {
                body.put_u8(EMPTY_PROPS);
            }
            for f in filters {
                put_string(&mut body, f);
            }
            0xA2
        }
        Packet::UnsubAck { pid, reasons } => {
            body.put_u16(*pid);
            if version == Version::V5 {
                body.put_u8(EMPTY_PROPS);
                for r in reasons {
                    body.put_u8(*r);
                }
            }
            0xB0
        }
        Packet::PingReq => 0xC0,
        Packet::PingResp => 0xD0,
        Packet::Disconnect { reason } => {
            if version == Version::V5 && *reason != 0 {
                body.put_u8(*reason);
                body.put_u8(EMPTY_PROPS);
            }
            0xE0
        }
    };

    dst.put_u8(flags);
    put_remaining_length(dst, body.len());
    dst.put_slice(&body);
    Ok(())
}

/// SUBACK body: packet id, v5 properties, one reason code per filter.
fn suback_body(version: Version, pid: u16, reasons: &[u8], body: &mut BytesMut) -> Result<u8> {
    body.put_u16(pid);
    if version == Version::V5 {
        body.put_u8(EMPTY_PROPS);
    }
    for r in reasons {
        body.put_u8(*r);
    }
    Ok(0x90)
}

fn encode_connect(version: Version, c: &Connect, body: &mut BytesMut) -> Result<()> {
    if c.client_id.len() > u16::MAX as usize {
        return Err(CodecError::Malformed("client id too long"));
    }
    let mut flags: u8 = 0;
    if c.clean {
        flags |= 0x02;
    }
    let username = match (&c.username, &c.password) {
        (Some(u), _) => Some(u.clone()),
        (None, Some(_)) => Some(String::new()), // a password requires the flag
        (None, None) => None,
    };
    if username.is_some() {
        flags |= 0x80;
    }
    if c.password.is_some() {
        flags |= 0x40;
    }
    if let Some(w) = &c.will {
        flags |= 0x04 | (w.qos.as_u8() << 3);
        if w.retain {
            flags |= 0x20;
        }
    }

    put_string(body, "MQTT");
    body.put_u8(version.level());
    body.put_u8(flags);
    body.put_u16(c.keepalive);

    if version == Version::V5 {
        let props = prop_len_u32(c.session_expiry);
        put_remaining_length(body, props);
        if let Some(se) = c.session_expiry {
            body.put_u8(0x11);
            body.put_u32(se);
        }
    }

    put_string(body, &c.client_id);
    if let Some(w) = &c.will {
        if version == Version::V5 {
            body.put_u8(EMPTY_PROPS);
        }
        put_string(body, &w.topic);
        put_binary(body, &w.payload);
    }
    if let Some(u) = &username {
        put_string(body, u);
    }
    if let Some(p) = &c.password {
        put_binary(body, p);
    }
    Ok(())
}

fn encode_publish(version: Version, p: &Publish, body: &mut BytesMut) -> Result<()> {
    if p.qos != QoS::AtMostOnce && p.pid.is_none() {
        return Err(CodecError::InvalidPacketId);
    }
    if p.qos == QoS::AtMostOnce && p.pid.is_some() {
        return Err(CodecError::Malformed("QoS 0 must not carry a packet id"));
    }
    put_string(body, &p.topic);
    if let Some(pid) = p.pid {
        if pid == 0 {
            return Err(CodecError::InvalidPacketId);
        }
        body.put_u16(pid);
    }
    if version == Version::V5 {
        body.put_u8(EMPTY_PROPS);
    }
    body.put_slice(&p.payload);
    Ok(())
}

/// PUBACK / PUBREC / PUBREL / PUBCOMP: id, then reason + properties when needed.
fn encode_ack(version: Version, pid: u16, reason: u8, body: &mut BytesMut) -> Result<()> {
    if pid == 0 {
        return Err(CodecError::InvalidPacketId);
    }
    body.put_u16(pid);
    if version == Version::V5 && reason != 0 {
        body.put_u8(reason);
        body.put_u8(EMPTY_PROPS);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{Buf, Bytes};

    fn bytes(version: Version, pkt: &Packet) -> Vec<u8> {
        let mut dst = BytesMut::new();
        encode(version, pkt, &mut dst).unwrap();
        dst.to_vec()
    }

    #[test]
    fn connect_matches_the_specification_example() {
        // MQTT 3.1.1 CONNECT, client "test", clean session, keepalive 60s.
        let got = bytes(
            Version::V3,
            &Packet::Connect(Connect::new("test", 60, true)),
        );
        assert_eq!(
            got,
            vec![
                0x10, 0x10, // fixed header + remaining length 16
                0x00, 0x04, b'M', b'Q', b'T', b'T', // protocol name
                0x04, // level 4
                0x02, // clean session
                0x00, 0x3C, // keepalive 60
                0x00, 0x04, b't', b'e', b's', b't', // client id
            ]
        );
    }

    #[test]
    fn connect_flags_and_will() {
        let c = Connect {
            will: Some(Will {
                topic: "w".into(),
                payload: b"bye".to_vec(),
                qos: QoS::AtLeastOnce,
                retain: true,
            }),
            username: Some("u".into()),
            password: Some(b"pw".to_vec()),
            ..Connect::new("id", 30, false)
        };
        let frame = bytes(Version::V3, &Packet::Connect(c));
        // fixed header(2) + name(6) + level(1) -> the connect flags sit at 9.
        assert_eq!(frame[2], 0x00, "reserved bits of the fixed header");
        assert_eq!(frame[4..8], *b"MQTT");
        assert_eq!(frame[8], 4, "protocol level");
        // will(0x04) | will qos 1 (0x08) | will retain(0x20) | user(0x80) | pass(0x40)
        assert_eq!(frame[9], 0x04 | 0x08 | 0x20 | 0x80 | 0x40);
        assert_eq!(
            &frame[frame.len() - 2..],
            b"pw",
            "password is the last field"
        );
        assert!(
            frame.windows(3).any(|w| w == b"\x00\x01w"),
            "will topic present"
        );
    }

    #[test]
    fn v5_connect_has_a_property_block() {
        let got = bytes(Version::V5, &Packet::Connect(Connect::new("ab", 10, true)));
        assert_eq!(got[2..6], [0x00, 0x04, b'M', b'Q'], "protocol name");
        assert_eq!(got[8], 5, "protocol level");
        assert_eq!(got[9], 0x02, "clean flag");
        assert_eq!(
            &got[12..],
            [0x00 /* property length */, 0x00, 0x02, b'a', b'b'],
            "{got:?}"
        );

        let c = Connect {
            session_expiry: Some(3600),
            ..Connect::new("ab", 10, false)
        };
        let got = bytes(Version::V5, &Packet::Connect(c));
        assert_eq!(got[12], 5, "one u32 property");
        assert_eq!(got[13], 0x11, "session expiry property id");
        assert_eq!(&got[14..18], &[0x00, 0x00, 0x0E, 0x10], "3600 as a u32");
    }

    #[test]
    fn publish_encoding() {
        let p = Publish {
            topic: "a/b".into(),
            payload: Bytes::from_static(b"hi"),
            qos: QoS::AtMostOnce,
            retain: false,
            dup: false,
            pid: None,
        };
        assert_eq!(
            bytes(Version::V3, &Packet::Publish(p.clone())),
            vec![0x30, 0x07, 0, 3, b'a', b'/', b'b', b'h', b'i']
        );

        let retained = Publish {
            retain: true,
            dup: true,
            ..p.clone()
        };
        let got = bytes(Version::V3, &Packet::Publish(retained));
        assert_eq!(got[0], 0x30 | 0x08 | 0x01);

        // QoS 1 carries the packet id.
        let q1 = Publish {
            qos: QoS::AtLeastOnce,
            pid: Some(7),
            ..p.clone()
        };
        let got = bytes(Version::V3, &Packet::Publish(q1.clone()));
        assert_eq!(got[0], 0x32);
        assert_eq!(&got[2..7], &[0, 3, b'a', b'/', b'b']);
        assert_eq!(&got[7..9], &[0, 7]);

        // v5 inserts an (empty) properties length.
        let got = bytes(Version::V5, &Packet::Publish(q1));
        assert_eq!(*got.last().unwrap(), b'i');
        assert_eq!(
            got[got.len() - 3],
            0x00,
            "empty property block before the payload"
        );
    }

    #[test]
    fn publish_validation() {
        let base = Publish {
            topic: "t".into(),
            payload: Bytes::new(),
            qos: QoS::AtMostOnce,
            retain: false,
            dup: false,
            pid: None,
        };
        let mut dst = BytesMut::new();
        // QoS 1 without an id is rejected...
        let q1 = Publish {
            qos: QoS::AtLeastOnce,
            ..base.clone()
        };
        assert_eq!(
            encode(Version::V3, &Packet::Publish(q1), &mut dst),
            Err(CodecError::InvalidPacketId)
        );
        // ... as is a zero id, and a stray id on QoS 0.
        let zero = Publish {
            qos: QoS::AtLeastOnce,
            pid: Some(0),
            ..base.clone()
        };
        assert_eq!(
            encode(Version::V3, &Packet::Publish(zero), &mut dst),
            Err(CodecError::InvalidPacketId)
        );
        let stray = Publish {
            pid: Some(1),
            ..base.clone()
        };
        assert_eq!(
            encode(Version::V3, &Packet::Publish(stray), &mut dst),
            Err(CodecError::Malformed("QoS 0 must not carry a packet id"))
        );
    }

    #[test]
    fn ack_packets_reuse_ids_only_within_rules() {
        assert_eq!(
            bytes(Version::V3, &Packet::PubAck { pid: 3, reason: 0 }),
            vec![0x40, 0x02, 0x00, 0x03]
        );
        // v5 success omits reason code and properties
        assert_eq!(
            bytes(Version::V5, &Packet::PubAck { pid: 3, reason: 0 }),
            vec![0x40, 0x02, 0x00, 0x03]
        );
        // v5 error carries reason + empty properties
        assert_eq!(
            bytes(
                Version::V5,
                &Packet::PubAck {
                    pid: 3,
                    reason: 0x87
                }
            ),
            vec![0x40, 0x04, 0x00, 0x03, 0x87, 0x00]
        );
        // PUBREL is packet type 6 with reserved flags 0010, and the reason
        // code the caller asked for is written verbatim.
        assert_eq!(
            bytes(Version::V3, &Packet::PubRel { pid: 3, reason: 2 }),
            vec![0x62, 0x02, 0x00, 0x03]
        );
        assert_eq!(
            bytes(Version::V5, &Packet::PubRel { pid: 3, reason: 2 }),
            vec![0x62, 0x04, 0x00, 0x03, 0x02, 0x00]
        );
        assert_eq!(
            bytes(Version::V5, &Packet::PubRel { pid: 3, reason: 0 }),
            vec![0x62, 0x02, 0x00, 0x03]
        );
        let mut dst = BytesMut::new();
        assert_eq!(
            encode(
                Version::V3,
                &Packet::PubComp { pid: 0, reason: 0 },
                &mut dst
            ),
            Err(CodecError::InvalidPacketId)
        );
    }

    #[test]
    fn subscribe_suback_and_small_packets() {
        let sub = Packet::Subscribe {
            pid: 1,
            filters: vec![(
                "a/#".into(),
                SubOpts {
                    qos: QoS::AtLeastOnce,
                },
            )],
        };
        assert_eq!(
            bytes(Version::V3, &sub),
            vec![0x82, 0x08, 0, 1, 0, 3, b'a', b'/', b'#', 1]
        );
        assert_eq!(
            bytes(Version::V5, &sub),
            vec![0x82, 0x09, 0, 1, 0, 0, 3, b'a', b'/', b'#', 1]
        );

        let suback = Packet::SubAck {
            pid: 1,
            reasons: vec![1],
        };
        assert_eq!(bytes(Version::V3, &suback), vec![0x90, 0x03, 0, 1, 1]);
        assert_eq!(bytes(Version::V5, &suback), vec![0x90, 0x04, 0, 1, 0, 1]);

        assert_eq!(bytes(Version::V3, &Packet::PingReq), vec![0xC0, 0x00]);
        assert_eq!(bytes(Version::V5, &Packet::PingResp), vec![0xD0, 0x00]);
        assert_eq!(
            bytes(Version::V3, &Packet::Disconnect { reason: 0 }),
            vec![0xE0, 0x00]
        );
        assert_eq!(
            bytes(Version::V5, &Packet::Disconnect { reason: 0x8A }),
            vec![0xE0, 0x02, 0x8A, 0x00]
        );

        let unsub = Packet::Unsubscribe {
            pid: 4,
            filters: vec!["x".into()],
        };
        assert_eq!(
            bytes(Version::V3, &unsub),
            vec![0xA2, 0x05, 0, 4, 0, 1, b'x']
        );
    }

    #[test]
    fn variable_length_integer_boundaries() {
        for (len, size) in [
            (0usize, 1),
            (127, 1),
            (128, 2),
            (16_383, 2),
            (16_384, 3),
            (2_097_151, 3),
            (2_097_152, 4),
        ] {
            assert_eq!(remaining_length_len(len), size, "len {len}");
            let mut dst = BytesMut::new();
            put_remaining_length(&mut dst, len);
            assert_eq!(dst.len(), size);
            let mut src = dst.freeze();
            let mut value: usize = 0;
            let mut shift = 0;
            while !src.is_empty() {
                let b = src[0];
                src.advance(1);
                value |= ((b & 0x7F) as usize) << shift;
                shift += 7;
                if b & 0x80 == 0 {
                    break;
                }
            }
            assert_eq!(value, len);
        }
    }

    #[test]
    fn sending_connack_is_not_supported() {
        let mut dst = BytesMut::new();
        assert!(matches!(
            encode(
                Version::V3,
                &Packet::ConnAck {
                    reason: 0,
                    session_present: false,
                    receive_max: None
                },
                &mut dst
            ),
            Err(CodecError::Unsupported(_))
        ));
    }

    #[test]
    fn wire_size_helper_matches_the_encoder() {
        for version in [Version::V3, Version::V5] {
            for qos in 0..3u8 {
                let p = Publish {
                    topic: "a/b".into(),
                    payload: Bytes::from(vec![b'x'; 40]),
                    qos: QoS::from_u8(qos),
                    retain: false,
                    dup: false,
                    pid: (qos > 0).then_some(5),
                };
                let got = bytes(version, &Packet::Publish(p));
                assert_eq!(
                    got.len(),
                    publish_wire_size(version, "a/b", 40, qos),
                    "{version:?} qos {qos}"
                );
            }
        }
    }
}
