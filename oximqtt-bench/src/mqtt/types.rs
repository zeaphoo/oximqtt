//! MQTT protocol types shared by the encoder and the decoder.
//!
//! This is an **independent** implementation of the wire format used only by
//! `mqtt-bench`. It deliberately does not depend on the `oximqtt` crate: a
//! benchmark that encodes and decodes with the system-under-test's own codec
//! cannot detect framing bugs and would cap the measured throughput at its own
//! (shared) codec speed. The packets are also reduced to what a load generator
//! needs — the version difference is a single flag, so the client state machine
//! above it stays protocol-version agnostic.

use std::fmt;

use bytes::Bytes;

/// Protocol level the frames are built for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    /// MQTT 3.1.1 (protocol level 4)
    V3,
    /// MQTT 5.0 (protocol level 5)
    V5,
}

impl Version {
    /// CONNECT protocol level field.
    pub const fn level(self) -> u8 {
        match self {
            Version::V3 => 4,
            Version::V5 => 5,
        }
    }
}

/// Quality of service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[allow(
    clippy::enum_variant_names,
    reason = "names mirror the MQTT delivery guarantees"
)]
pub enum QoS {
    /// Fire and forget.
    AtMostOnce = 0,
    /// At least once, confirmed by PUBACK.
    AtLeastOnce = 1,
    /// Exactly once, confirmed by the PUBREC/PUBREL/PUBCOMP exchange.
    ExactlyOnce = 2,
}

impl QoS {
    /// Parse the numeric level, anything unknown becomes QoS 2 (safest).
    pub const fn from_u8(v: u8) -> QoS {
        match v {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            _ => QoS::ExactlyOnce,
        }
    }

    /// Numeric level.
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

impl fmt::Display for QoS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_u8())
    }
}

/// Subscription options (the v5 byte carries more bits, unused here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubOpts {
    /// Requested maximum QoS.
    pub qos: QoS,
}

/// Last will and testament.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Will {
    /// Will topic.
    pub topic: String,
    /// Will payload.
    pub payload: Vec<u8>,
    /// Will QoS.
    pub qos: QoS,
    /// Will retain flag.
    pub retain: bool,
}

/// CONNECT packet content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connect {
    /// Clean session (v3.1.1) / Clean start (v5).
    pub clean: bool,
    /// Keepalive in seconds.
    pub keepalive: u16,
    /// Client identifier.
    pub client_id: String,
    /// Optional user name.
    pub username: Option<String>,
    /// Optional password.
    pub password: Option<Vec<u8>>,
    /// Optional last will.
    pub will: Option<Will>,
    /// v5 only: session expiry interval in seconds, `Some` writes the property.
    pub session_expiry: Option<u32>,
}

impl Connect {
    /// CONNECT with only the mandatory fields.
    pub fn new(client_id: impl Into<String>, keepalive: u16, clean: bool) -> Self {
        Connect {
            clean,
            keepalive,
            client_id: client_id.into(),
            username: None,
            password: None,
            will: None,
            session_expiry: None,
        }
    }
}

/// PUBLISH packet content.
#[derive(Clone, PartialEq, Eq)]
pub struct Publish {
    /// Topic name.
    pub topic: String,
    /// Application message.
    pub payload: Bytes,
    /// Delivery QoS.
    pub qos: QoS,
    /// Retain flag.
    pub retain: bool,
    /// Duplicate delivery flag.
    pub dup: bool,
    /// Packet identifier, present for QoS 1 and 2.
    pub pid: Option<u16>,
}

impl fmt::Debug for Publish {
    /// Never dump message payloads, they are usually large.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Publish")
            .field("topic", &self.topic)
            .field("qos", &self.qos)
            .field("retain", &self.retain)
            .field("dup", &self.dup)
            .field("pid", &self.pid)
            .field("payload_len", &self.payload.len())
            .finish()
    }
}

/// Every packet the benchmark can send or receive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Packet {
    /// CONNECT
    Connect(Connect),
    /// CONNACK; `reason` is 0 on success in both protocol versions.
    /// `receive_max` is the broker's v5 Receive Maximum property (0x21),
    /// which caps how many QoS 1/2 messages we may have in flight.
    ConnAck {
        reason: u8,
        session_present: bool,
        receive_max: Option<u16>,
    },
    /// PUBLISH
    Publish(Publish),
    /// PUBACK
    PubAck { pid: u16, reason: u8 },
    /// PUBREC
    PubRec { pid: u16, reason: u8 },
    /// PUBREL
    PubRel { pid: u16, reason: u8 },
    /// PUBCOMP
    PubComp { pid: u16, reason: u8 },
    /// SUBSCRIBE
    Subscribe {
        pid: u16,
        filters: Vec<(String, SubOpts)>,
    },
    /// SUBACK, one reason code per filter (0x80 and above mean failure)
    SubAck { pid: u16, reasons: Vec<u8> },
    /// UNSUBSCRIBE (kept for completeness, the churn controller may use it)
    Unsubscribe { pid: u16, filters: Vec<String> },
    /// UNSUBACK
    UnsubAck { pid: u16, reasons: Vec<u8> },
    /// PINGREQ
    PingReq,
    /// PINGRESP
    PingResp,
    /// DISCONNECT
    Disconnect { reason: u8 },
}

impl Packet {
    /// Packet type nibble, for tests and logging.
    pub const fn packet_type(&self) -> u8 {
        match self {
            Packet::Connect(_) => 1,
            Packet::ConnAck { .. } => 2,
            Packet::Publish(_) => 3,
            Packet::PubAck { .. } => 4,
            Packet::PubRec { .. } => 5,
            Packet::PubRel { .. } => 6,
            Packet::PubComp { .. } => 7,
            Packet::Subscribe { .. } => 8,
            Packet::SubAck { .. } => 9,
            Packet::Unsubscribe { .. } => 10,
            Packet::UnsubAck { .. } => 11,
            Packet::PingReq => 12,
            Packet::PingResp => 13,
            Packet::Disconnect { .. } => 14,
        }
    }
}

/// Codec failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// The frame exceeds the configured size limit.
    PacketTooLarge { size: usize, limit: usize },
    /// QoS 1/2 PUBLISH without a packet identifier, or a zero identifier.
    InvalidPacketId,
    /// Reserved/unknown QoS, flag combinations or protocol values.
    Malformed(&'static str),
    /// The frame ended in the middle of a field.
    Incomplete,
    /// A packet this codec cannot produce for the negotiated version.
    Unsupported(&'static str),
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecError::PacketTooLarge { size, limit } => {
                write!(f, "packet size {size} exceeds limit {limit}")
            }
            CodecError::InvalidPacketId => write!(f, "invalid packet identifier"),
            CodecError::Malformed(what) => write!(f, "malformed packet: {what}"),
            CodecError::Incomplete => write!(f, "incomplete packet"),
            CodecError::Unsupported(what) => write!(f, "unsupported: {what}"),
        }
    }
}

impl std::error::Error for CodecError {}

/// Result alias for codec operations.
pub type Result<T> = std::result::Result<T, CodecError>;
