//! MQTT packet decoder.

use crate::mqtt::types::*;

/// Cursor over a complete packet body.
struct Cur<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cur<'a> {
    fn new(data: &'a [u8]) -> Self {
        Cur { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    fn byte(&mut self) -> Result<u8> {
        let b = *self.data.get(self.pos).ok_or(CodecError::Incomplete)?;
        self.pos += 1;
        Ok(b)
    }

    fn u16(&mut self) -> Result<u16> {
        if self.remaining() < 2 {
            return Err(CodecError::Incomplete);
        }
        let v = u16::from_be_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    /// Variable length integer (up to 4 bytes, 28 bits).
    fn varint(&mut self) -> Result<usize> {
        let mut value: usize = 0;
        let mut shift = 0;
        for _ in 0..4 {
            let b = self.byte()?;
            value |= ((b & 0x7F) as usize) << shift;
            if b & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
        }
        Err(CodecError::Malformed("variable length integer too long"))
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.remaining() < n {
            return Err(CodecError::Incomplete);
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn string(&mut self) -> Result<String> {
        let len = self.u16()? as usize;
        let raw = self.take(len)?;
        String::from_utf8(raw.to_vec()).map_err(|_| CodecError::Malformed("invalid utf-8 string"))
    }

    fn binary(&mut self) -> Result<Vec<u8>> {
        let len = self.u16()? as usize;
        Ok(self.take(len)?.to_vec())
    }

    fn rest(&mut self) -> &'a [u8] {
        let s = &self.data[self.pos..];
        self.pos = self.data.len();
        s
    }

    /// Skip a v5 property block and return its declared length.
    fn skip_properties(&mut self) -> Result<usize> {
        let len = self.varint()?;
        if len > self.remaining() {
            return Err(CodecError::Incomplete);
        }
        self.pos += len;
        Ok(len)
    }
}

/// Parse a packet body. `header` is the fixed header byte, `body` the frame
/// payload with the remaining length already verified.
pub fn parse(version: Version, header: u8, body: &[u8]) -> Result<Packet> {
    let kind = header >> 4;
    let flags = header & 0x0F;
    let mut c = Cur::new(body);
    match kind {
        1 => parse_connect(version, flags, &mut c),
        2 => parse_connack(version, &mut c),
        3 => parse_publish(version, flags, &mut c),
        4 => parse_ack(&mut c, version, |pid, reason| Packet::PubAck {
            pid,
            reason,
        }),
        5 => parse_ack(&mut c, version, |pid, reason| Packet::PubRec {
            pid,
            reason,
        }),
        6 => {
            if flags != 0x02 {
                return Err(CodecError::Malformed("PUBREL reserved flags"));
            }
            parse_ack(&mut c, version, |pid, reason| Packet::PubRel {
                pid,
                reason,
            })
        }
        7 => parse_ack(&mut c, version, |pid, reason| Packet::PubComp {
            pid,
            reason,
        }),
        8 => parse_subscribe(version, flags, &mut c),
        9 => parse_suback(version, &mut c),
        10 => parse_unsubscribe(version, flags, &mut c),
        11 => parse_unsuback(version, &mut c),
        12 => Ok(Packet::PingReq),
        13 => Ok(Packet::PingResp),
        14 => {
            let reason = if version == Version::V5 && c.remaining() > 0 {
                c.byte()?
            } else {
                0
            };
            Ok(Packet::Disconnect { reason })
        }
        _ => Err(CodecError::Malformed("reserved packet type")),
    }
}

fn parse_connect(version: Version, header_flags: u8, c: &mut Cur) -> Result<Packet> {
    let name = c.string()?;
    if name != "MQTT" && name != "MQIsdp" {
        return Err(CodecError::Malformed("bad protocol name"));
    }
    let level = c.byte()?;
    if !matches!(level, 3..=5) {
        return Err(CodecError::Malformed("bad protocol level"));
    }
    if version == Version::V3 && header_flags & 0x0F != 0 {
        return Err(CodecError::Malformed("reserved connect flags"));
    }
    // The connect flags live in the variable header, not the fixed header.
    let flags = c.byte()?;
    let keepalive = c.u16()?;
    if version == Version::V5 {
        c.skip_properties()?;
    }
    let client_id = c.string()?;
    let will = if flags & 0x04 != 0 {
        if version == Version::V5 {
            c.skip_properties()?;
        }
        let topic = c.string()?;
        let payload = c.binary()?;
        Some(Will {
            topic,
            payload,
            qos: QoS::from_u8((flags >> 3) & 0x03),
            retain: flags & 0x20 != 0,
        })
    } else {
        None
    };
    let username = if flags & 0x80 != 0 {
        Some(c.string()?)
    } else {
        None
    };
    let password = if flags & 0x40 != 0 {
        Some(c.binary()?)
    } else {
        None
    };
    Ok(Packet::Connect(Connect {
        clean: flags & 0x02 != 0,
        keepalive,
        client_id,
        username,
        password,
        will,
        session_expiry: None,
    }))
}

fn parse_connack(version: Version, c: &mut Cur) -> Result<Packet> {
    if version == Version::V3 {
        let session_present = c.byte()? & 0x01 != 0;
        let reason = c.byte()?;
        return Ok(Packet::ConnAck {
            reason,
            session_present,
            receive_max: None,
        });
    }
    let reason = c.byte()?;
    let receive_max = parse_connack_properties(c)?;
    Ok(Packet::ConnAck {
        reason,
        session_present: false,
        receive_max,
    })
}

/// Read the v5 CONNACK property block, capturing the Receive Maximum (0x21).
/// Every property id the specification allows in a CONNACK is understood, so
/// the block is skipped correctly even when the broker sends several.
fn parse_connack_properties(c: &mut Cur) -> Result<Option<u16>> {
    let block = c.varint()?;
    if block > c.remaining() {
        return Err(CodecError::Incomplete);
    }
    let end = c.pos + block;
    let mut receive_max = None;
    while c.pos < end {
        match c.byte()? {
            0x11 | 0x13 => {
                c.take(4)?; // session expiry / server keep alive (u32)
            }
            0x12 => {
                // assigned client identifier (utf-8)
                let n = c.varint()?;
                c.take(n)?;
            }
            0x1A | 0x1C | 0x1F => {
                // response info / server reference / reason string
                let n = c.varint()?;
                c.take(n)?;
            }
            0x21 => receive_max = Some(c.u16()?),
            0x22 => {
                c.u16()?; // topic alias maximum
            }
            0x24 | 0x25 | 0x28 | 0x29 | 0x2A => {
                c.byte()?; // byte flags
            }
            0x26 => {
                // user property: two utf-8 strings
                let n = c.varint()?;
                c.take(n)?;
                let n = c.varint()?;
                c.take(n)?;
            }
            0x27 => {
                c.take(4)?; // maximum packet size (u32)
            }
            _unknown => return Err(CodecError::Malformed("unknown CONNACK property")),
        }
    }
    if c.pos > end {
        return Err(CodecError::Malformed("CONNACK property overflow"));
    }
    Ok(receive_max)
}

fn parse_publish(version: Version, flags: u8, c: &mut Cur) -> Result<Packet> {
    let qos_raw = (flags >> 1) & 0x03;
    if qos_raw > 2 {
        return Err(CodecError::Malformed("reserved QoS value"));
    }
    let qos = QoS::from_u8(qos_raw);
    let topic = c.string()?;
    if topic.is_empty() {
        return Err(CodecError::Malformed("empty topic name"));
    }
    let pid = if qos == QoS::AtMostOnce {
        None
    } else {
        Some(read_pid(c)?)
    };
    if version == Version::V5 {
        c.skip_properties()?;
    }
    let payload = bytes::Bytes::copy_from_slice(c.rest());
    Ok(Packet::Publish(Publish {
        topic,
        payload,
        qos,
        retain: flags & 0x01 != 0,
        dup: flags & 0x08 != 0,
        pid,
    }))
}

fn read_pid(c: &mut Cur) -> Result<u16> {
    let pid = c.u16()?;
    if pid == 0 {
        return Err(CodecError::InvalidPacketId);
    }
    Ok(pid)
}

/// Decode `pid` (+ v5 reason code / properties) and build an ack packet.
fn parse_ack(c: &mut Cur, version: Version, build: impl Fn(u16, u8) -> Packet) -> Result<Packet> {
    let pid = read_pid(c)?;
    let mut reason = 0u8;
    if version == Version::V5 && c.remaining() > 0 {
        reason = c.byte()?;
        if c.remaining() > 0 {
            c.skip_properties()?;
        }
    }
    Ok(build(pid, reason))
}

fn parse_subscribe(version: Version, flags: u8, c: &mut Cur) -> Result<Packet> {
    if flags != 0x02 {
        return Err(CodecError::Malformed("SUBSCRIBE reserved flags"));
    }
    let pid = read_pid(c)?;
    if version == Version::V5 {
        c.skip_properties()?;
    }
    let mut filters = Vec::new();
    while c.remaining() > 0 {
        let filter = c.string()?;
        let byte = c.byte()?;
        if version == Version::V3 && byte & 0xFC != 0 {
            return Err(CodecError::Malformed("subscription options"));
        }
        filters.push((
            filter,
            SubOpts {
                qos: QoS::from_u8(byte & 0x03),
            },
        ));
    }
    if filters.is_empty() {
        return Err(CodecError::Malformed("SUBSCRIBE without filters"));
    }
    Ok(Packet::Subscribe { pid, filters })
}

fn parse_suback(version: Version, c: &mut Cur) -> Result<Packet> {
    let pid = read_pid(c)?;
    if version == Version::V5 {
        c.skip_properties()?;
    }
    let reasons = c.rest().to_vec();
    Ok(Packet::SubAck { pid, reasons })
}

fn parse_unsubscribe(version: Version, flags: u8, c: &mut Cur) -> Result<Packet> {
    if flags != 0x02 {
        return Err(CodecError::Malformed("UNSUBSCRIBE reserved flags"));
    }
    let pid = read_pid(c)?;
    if version == Version::V5 {
        c.skip_properties()?;
    }
    let mut filters = Vec::new();
    while c.remaining() > 0 {
        filters.push(c.string()?);
    }
    Ok(Packet::Unsubscribe { pid, filters })
}

fn parse_unsuback(version: Version, c: &mut Cur) -> Result<Packet> {
    let pid = read_pid(c)?;
    let mut reasons = Vec::new();
    if version == Version::V5 {
        c.skip_properties()?;
        reasons = c.rest().to_vec();
    }
    Ok(Packet::UnsubAck { pid, reasons })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mqtt::encode::encode;
    use bytes::{Bytes, BytesMut};

    /// Encode, then parse the frame back (the framing is verified by the
    /// `mqtt::tests` stream tests; here we focus on the field layout).
    fn roundtrip(version: Version, pkt: &Packet) -> Packet {
        let mut dst = BytesMut::new();
        encode(version, pkt, &mut dst).unwrap();
        parse_frame(version, &dst)
    }

    /// Parse a complete frame from its wire bytes, server packets included.
    fn parse_frame(version: Version, frame: &[u8]) -> Packet {
        let mut cur = Cur::new(&frame[1..]);
        let len = cur.varint().unwrap();
        assert_eq!(
            cur.remaining(),
            len,
            "declared remaining length must match the frame"
        );
        let body = cur.take(len).unwrap();
        parse(version, frame[0], body).unwrap()
    }

    #[test]
    fn roundtrips_connect() {
        for version in [Version::V3, Version::V5] {
            let c = Connect {
                username: Some("user".into()),
                password: Some(b"secret".to_vec()),
                will: Some(Will {
                    topic: "w/t".into(),
                    payload: b"gone".to_vec(),
                    qos: QoS::ExactlyOnce,
                    retain: true,
                }),
                ..Connect::new("client-1", 120, true)
            };
            let back = roundtrip(version, &Packet::Connect(c.clone()));
            match back {
                Packet::Connect(got) => {
                    assert_eq!(got.client_id, "client-1");
                    assert_eq!(got.keepalive, 120);
                    assert!(got.clean);
                    assert_eq!(got.username.as_deref(), Some("user"));
                    assert_eq!(got.password.as_deref(), Some(&b"secret"[..]));
                    let w = got.will.expect("will survived");
                    assert_eq!(w.topic, "w/t");
                    assert_eq!(w.qos, QoS::ExactlyOnce);
                    assert!(w.retain);
                }
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn roundtrips_publish_for_every_qos() {
        for version in [Version::V3, Version::V5] {
            for qos in 0..=2u8 {
                let p = Publish {
                    topic: "sensor/1".into(),
                    payload: Bytes::from(b"hello world".to_vec()),
                    qos: QoS::from_u8(qos),
                    retain: qos == 2,
                    dup: qos == 1,
                    pid: (qos > 0).then_some(3131),
                };
                let pkt = Packet::Publish(p.clone());
                let back = roundtrip(version, &pkt);
                assert_eq!(back, pkt, "{version:?} qos {qos}");
            }
        }
    }

    #[test]
    fn roundtrips_control_packets() {
        for version in [Version::V3, Version::V5] {
            let pkts = vec![
                Packet::PubAck { pid: 1, reason: 0 },
                Packet::PubAck {
                    pid: 1,
                    reason: 0x92,
                },
                Packet::PubRec { pid: 2, reason: 0 },
                Packet::PubRel { pid: 3, reason: 2 },
                Packet::PubComp { pid: 4, reason: 0 },
                Packet::Subscribe {
                    pid: 5,
                    filters: vec![
                        (
                            "a/#".into(),
                            SubOpts {
                                qos: QoS::AtMostOnce,
                            },
                        ),
                        (
                            "b/+".into(),
                            SubOpts {
                                qos: QoS::ExactlyOnce,
                            },
                        ),
                    ],
                },
                Packet::SubAck {
                    pid: 5,
                    reasons: vec![0, 2],
                },
                Packet::Unsubscribe {
                    pid: 6,
                    filters: vec!["a/#".into()],
                },
                Packet::UnsubAck {
                    pid: 6,
                    reasons: vec![0],
                },
                Packet::PingReq,
                Packet::PingResp,
                Packet::Disconnect { reason: 0 },
            ];
            for p in pkts {
                let back = roundtrip(version, &p);
                assert_eq!(back, normalize(version, &p), "{version:?}");
            }
        }
    }

    /// v3.1.1 has no reason codes at all: acks and unsubacks come back clean.
    fn normalize(version: Version, pkt: &Packet) -> Packet {
        if version != Version::V3 {
            return pkt.clone();
        }
        match pkt {
            Packet::PubAck { pid, .. } => Packet::PubAck {
                pid: *pid,
                reason: 0,
            },
            Packet::PubRel { pid, .. } => Packet::PubRel {
                pid: *pid,
                reason: 0,
            },
            Packet::UnsubAck { pid, .. } => Packet::UnsubAck {
                pid: *pid,
                reasons: vec![],
            },
            other => other.clone(),
        }
    }

    #[test]
    fn connack_frames_are_parsed_for_both_versions() {
        // v3.1.1: session present flag + return code.
        let v3 = parse_frame(Version::V3, &[0x20, 0x02, 0x01, 0x00]);
        assert_eq!(
            v3,
            Packet::ConnAck {
                reason: 0,
                session_present: true,
                receive_max: None
            }
        );
        let v3 = parse_frame(Version::V3, &[0x20, 0x02, 0x00, 0x05]);
        assert_eq!(
            v3,
            Packet::ConnAck {
                reason: 5,
                session_present: false,
                receive_max: None
            }
        );
        // v5: reason code + empty properties.
        let v5 = parse_frame(Version::V5, &[0x20, 0x02, 0x8F, 0x00]);
        assert_eq!(
            v5,
            Packet::ConnAck {
                reason: 0x8F,
                session_present: false,
                receive_max: None
            }
        );
        // v5 with a real property block (receive maximum = 10).
        let v5 = parse_frame(Version::V5, &[0x20, 0x05, 0x00, 0x03, 0x21, 0x00, 0x0A]);
        assert_eq!(
            v5,
            Packet::ConnAck {
                reason: 0,
                session_present: false,
                receive_max: Some(10)
            }
        );
        // v5 with several properties mixed in, receive max last.
        let v5 = parse_frame(
            Version::V5,
            &[
                0x20, 0x13, // remaining length 19
                0x00, // reason ok
                0x11, // property block: 17 bytes
                0x24, 0x01, // maximum qos = 1
                0x21, 0x00, 0x10, // receive maximum = 16
                0x22, 0x00, 0x20, // topic alias maximum = 32
                0x13, 0x00, 0x00, 0x00, 0x3C, // server keep alive = 60
                0x1F, 0x02, b'o', b'k', // reason string "ok"
            ],
        );
        assert_eq!(
            v5,
            Packet::ConnAck {
                reason: 0,
                session_present: false,
                receive_max: Some(16)
            }
        );
    }

    #[test]
    fn pubrel_requires_its_reserved_bits() {
        let err = parse(Version::V3, 0x60, &[0x00, 0x01]).unwrap_err();
        assert_eq!(err, CodecError::Malformed("PUBREL reserved flags"));
        assert!(matches!(
            parse(Version::V3, 0x62, &[0x00, 0x01]),
            Ok(Packet::PubRel { .. })
        ));
    }

    #[test]
    fn connect_flags_are_read_from_the_variable_header() {
        // client id "test", clean session, keepalive 60 -> the exact frame from
        // the MQTT 3.1.1 specification examples.
        let wire: &[u8] = &[
            0x10, 0x10, 0x00, 0x04, b'M', b'Q', b'T', b'T', 0x04, 0x02, 0x00, 0x3C, 0x00, 0x04,
            b't', b'e', b's', b't',
        ];
        let mut c = Cur::new(&wire[2..]);
        let pkt = parse(Version::V3, wire[0], c.take(16).unwrap()).unwrap();
        match pkt {
            Packet::Connect(c) => {
                assert_eq!(c.keepalive, 60, "keepalive must not eat the flags byte");
                assert!(c.clean);
                assert_eq!(c.client_id, "test");
                assert!(c.username.is_none() && c.password.is_none() && c.will.is_none());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn v5_ack_reason_codes_are_read() {
        let back = roundtrip(
            Version::V5,
            &Packet::PubAck {
                pid: 9,
                reason: 0x87,
            },
        );
        assert_eq!(
            back,
            Packet::PubAck {
                pid: 9,
                reason: 0x87
            }
        );
        // v3 has no reason codes at all.
        let back = roundtrip(Version::V3, &Packet::PubComp { pid: 9, reason: 0 });
        assert_eq!(back, Packet::PubComp { pid: 9, reason: 0 });
    }

    #[test]
    fn cursor_helpers() {
        let mut c = Cur::new(&[0x01, 0x02, 0x03]);
        assert_eq!(c.remaining(), 3);
        assert_eq!(c.byte().unwrap(), 1);
        assert_eq!(c.u16().unwrap(), 0x0203);
        assert_eq!(c.remaining(), 0);
        assert_eq!(c.byte().unwrap_err(), CodecError::Incomplete);
        assert_eq!(c.u16().unwrap_err(), CodecError::Incomplete);
        assert_eq!(c.varint().unwrap_err(), CodecError::Incomplete);

        // 4-byte varint with the continuation bit set on the last byte.
        let mut c = Cur::new(&[0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(
            c.varint().unwrap_err(),
            CodecError::Malformed("variable length integer too long")
        );
    }
}
