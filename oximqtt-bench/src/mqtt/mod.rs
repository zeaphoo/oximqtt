//! A self-contained MQTT 3.1.1 / 5.0 codec for the benchmark.
//!
//! Only the frames a load generator needs are implemented, but they are
//! implemented from the specification — including the reserved-bit and packet
//! id rules — so `mqtt-bench` exercises the broker as an external client would
//! instead of agreeing with the broker's own codec by construction.

pub mod decode;
pub mod encode;
pub mod types;

pub use encode::publish_wire_size;
pub use types::*;

use bytes::BytesMut;

/// Frame codec for one connection.
pub struct Codec {
    version: Version,
    max_packet_size: usize,
    /// Size of the frame we are currently waiting for, used to right-size reads.
    want: usize,
}

impl Codec {
    /// Codec for `version`, rejecting inbound frames above `max_packet_size`.
    pub fn new(version: Version, max_packet_size: usize) -> Self {
        Codec {
            version,
            max_packet_size: max_packet_size.max(64),
            want: 0,
        }
    }

    /// Bytes the codec expects for the partially decoded frame, `0` when it is
    /// at a frame boundary. Used to avoid over-reading.
    pub fn frame_hint(&self) -> usize {
        self.want
    }

    /// Serialize a packet, appending it to `dst`.
    pub fn encode(&mut self, pkt: &Packet, dst: &mut BytesMut) -> Result<()> {
        let before = dst.len();
        encode::encode(self.version, pkt, dst).inspect_err(|_| {
            dst.truncate(before);
        })?;
        if dst.len() - before > self.max_packet_size {
            dst.truncate(before);
            return Err(CodecError::PacketTooLarge {
                size: dst.len() - before,
                limit: self.max_packet_size,
            });
        }
        Ok(())
    }

    /// Try to decode one packet from `src`, consuming it when complete.
    ///
    /// Returns `Ok(None)` when more bytes are needed; the buffer is left
    /// untouched in that case.
    pub fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Packet>> {
        if src.len() < 2 {
            self.want = 2;
            return Ok(None);
        }
        let (len, header_len) = match peek_remaining_length(&src[1..])? {
            Some(v) => v,
            None => {
                self.want = src.len() + 1; // need at least one more header byte
                return Ok(None);
            }
        };
        let total = 1 + header_len + len;
        if len > self.max_packet_size {
            return Err(CodecError::PacketTooLarge {
                size: len,
                limit: self.max_packet_size,
            });
        }
        if src.len() < total {
            self.want = total;
            return Ok(None);
        }
        self.want = 0;

        let frame = src.split_to(total);
        let header = frame[0];
        let body = &frame[1 + header_len..];
        decode::parse(self.version, header, body).map(Some)
    }
}

/// Read the remaining-length varint without consuming: `(length, bytes used)`
/// or `None` when the varint is not complete yet.
fn peek_remaining_length(src: &[u8]) -> Result<Option<(usize, usize)>> {
    let mut value: usize = 0;
    let mut shift = 0;
    for (i, b) in src.iter().enumerate() {
        value |= ((b & 0x7F) as usize) << shift;
        shift += 7;
        if b & 0x80 == 0 {
            return Ok(Some((value, i + 1)));
        }
        if i == 3 {
            return Err(CodecError::Malformed("variable length integer too long"));
        }
    }
    Ok(None)
}

/// Encode a packet into a fresh buffer (convenience for tests and tooling).
#[cfg(test)]
pub fn encode_packet(version: Version, pkt: &Packet) -> Result<BytesMut> {
    let mut buf = BytesMut::with_capacity(64);
    Codec::new(version, 1024 * 1024).encode(pkt, &mut buf)?;
    Ok(buf)
}

/// Decode packets from a byte stream until it is exhausted.
#[cfg(test)]
pub fn decode_all(version: Version, buf: &mut BytesMut) -> Result<Vec<Packet>> {
    let mut codec = Codec::new(version, 1024 * 1024);
    let mut out = Vec::new();
    while let Some(pkt) = codec.decode(buf)? {
        out.push(pkt);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{Buf, BufMut, Bytes};

    fn samples() -> Vec<Packet> {
        vec![
            Packet::Connect(Connect::new("c", 60, true)),
            Packet::ConnAck {
                reason: 0,
                session_present: false,
                receive_max: Some(16),
            },
            Packet::Publish(Publish {
                topic: "a/b".into(),
                payload: Bytes::from_static(b"payload"),
                qos: QoS::AtMostOnce,
                retain: false,
                dup: false,
                pid: None,
            }),
            Packet::Publish(Publish {
                topic: "a/b".into(),
                payload: Bytes::from(vec![7u8; 300]),
                qos: QoS::ExactlyOnce,
                retain: true,
                dup: false,
                pid: Some(600),
            }),
            Packet::PubAck { pid: 12, reason: 0 },
            Packet::PubRel { pid: 12, reason: 2 },
            Packet::Subscribe {
                pid: 1,
                filters: vec![(
                    "s/#".into(),
                    SubOpts {
                        qos: QoS::AtLeastOnce,
                    },
                )],
            },
            Packet::SubAck {
                pid: 1,
                reasons: vec![1],
            },
            Packet::Unsubscribe {
                pid: 2,
                filters: vec!["s/#".into()],
            },
            Packet::UnsubAck {
                pid: 2,
                reasons: vec![],
            },
            Packet::PingReq,
            Packet::PingResp,
            Packet::Disconnect { reason: 0 },
        ]
    }

    #[test]
    fn stream_of_packets_decodes_in_one_go() {
        for version in [Version::V3, Version::V5] {
            let mut buf = BytesMut::new();
            let mut codec = Codec::new(version, 1024 * 1024);
            let mut expected = Vec::new();
            for p in samples() {
                let mut dst = BytesMut::new();
                if codec.encode(&p, &mut dst).is_err() {
                    continue; // CONNACK cannot be sent
                }
                expected.push(p.clone());
                buf.unsplit(dst);
            }
            let got = decode_all(version, &mut buf).unwrap();
            let expected: Vec<Packet> = expected
                .iter()
                .map(|p| match (version, p) {
                    (Version::V3, Packet::PubRel { pid, .. }) => Packet::PubRel {
                        pid: *pid,
                        reason: 0,
                    },
                    (_, other) => other.clone(),
                })
                .collect();
            assert_eq!(got, expected, "{version:?}");
            assert!(buf.is_empty());
        }
    }

    #[test]
    fn decoding_waits_for_the_whole_frame() {
        let pkt = Packet::Publish(Publish {
            topic: "t".into(),
            payload: Bytes::from(vec![b'z'; 200]),
            qos: QoS::AtLeastOnce,
            retain: false,
            dup: false,
            pid: Some(5),
        });
        let mut full = encode_packet(Version::V3, &pkt).unwrap();

        // Feed the frame byte by byte: nothing may be emitted until it is complete.
        let mut partial = BytesMut::new();
        let mut codec = Codec::new(Version::V3, 1024 * 1024);
        let mut emitted = 0;
        while !full.is_empty() {
            partial.put_u8(full.get_u8());
            if codec.decode(&mut partial).unwrap().is_some() {
                emitted += 1;
            }
        }
        assert_eq!(emitted, 1);
        assert!(
            partial.is_empty(),
            "the consumed frame must be taken from the buffer"
        );
    }

    #[test]
    fn frame_hint_tracks_the_pending_frame() {
        let pkt = Packet::Publish(Publish {
            topic: "t".into(),
            payload: Bytes::from(vec![0u8; 500]),
            qos: QoS::AtMostOnce,
            retain: false,
            dup: false,
            pid: None,
        });
        let total = encode_packet(Version::V3, &pkt).unwrap().len();
        let mut buf = BytesMut::new();
        let mut codec = Codec::new(Version::V3, 1024 * 1024);
        codec.encode(&pkt, &mut buf).unwrap();

        // Only the fixed header is available: the codec reports the size of
        // the whole frame it is waiting for and consumes nothing.
        let mut head = buf.split_to(3);
        assert!(codec.decode(&mut head).unwrap().is_none());
        assert_eq!(codec.frame_hint(), total);
        assert_eq!(head.len(), 3, "a partial frame must stay in the buffer");

        // Once the frame is complete the hint resets.
        head.unsplit(buf);
        let mut buf = head;
        assert!(codec.decode(&mut buf).unwrap().is_some());
        assert_eq!(codec.frame_hint(), 0);
    }

    #[test]
    fn oversized_packets_are_rejected() {
        let mut buf = BytesMut::new();
        // Hand-craft a 3-byte header announcing a 500 byte frame.
        buf.put_u8(0x30);
        buf.put_u8(0xAC);
        buf.put_u8(0x03);
        buf.put_slice(&[0u8; 500]);

        let mut codec = Codec::new(Version::V3, 64);
        let err = codec.decode(&mut buf).unwrap_err();
        assert!(matches!(err, CodecError::PacketTooLarge { .. }), "{err:?}");

        // Encoding is limited too, and leaves the buffer untouched.
        let big = Packet::Publish(Publish {
            topic: "t".into(),
            payload: Bytes::from(vec![0u8; 500]),
            qos: QoS::AtMostOnce,
            retain: false,
            dup: false,
            pid: None,
        });
        let mut out = BytesMut::from(&[1u8, 2, 3][..]);
        let mut codec = Codec::new(Version::V3, 64);
        assert!(codec.encode(&big, &mut out).is_err());
        assert_eq!(&out[..], &[1, 2, 3]);
    }

    #[test]
    fn minimum_codec_size_is_enforced() {
        let mut codec = Codec::new(Version::V3, 0);
        assert_eq!(codec.max_packet_size, 64);
        // The minimum still frames a PINGREQ.
        let mut buf = BytesMut::new();
        codec.encode(&Packet::PingReq, &mut buf).unwrap();
        assert_eq!(&buf[..], &[0xC0, 0x00]);
    }

    #[test]
    fn varint_peeking() {
        assert_eq!(peek_remaining_length(&[0x00]).unwrap(), Some((0, 1)));
        assert_eq!(peek_remaining_length(&[0x7F]).unwrap(), Some((127, 1)));
        assert_eq!(
            peek_remaining_length(&[0xFF, 0x7F]).unwrap(),
            Some((16383, 2))
        );
        assert_eq!(
            peek_remaining_length(&[0xFF, 0xFF, 0x7F]).unwrap(),
            Some((2097151, 3))
        );
        assert_eq!(peek_remaining_length(&[0x80]).unwrap(), None);
        assert_eq!(peek_remaining_length(&[0x80, 0x80, 0x80]).unwrap(), None);
        assert!(peek_remaining_length(&[0x80, 0x80, 0x80, 0x80, 0x01]).is_err());
    }

    #[test]
    fn empty_buffer_needs_two_bytes() {
        let mut buf = BytesMut::from(&[0x30u8][..]);
        let mut codec = Codec::new(Version::V3, 1024);
        assert!(codec.decode(&mut buf).unwrap().is_none());
        assert_eq!(codec.frame_hint(), 2);
    }
}
