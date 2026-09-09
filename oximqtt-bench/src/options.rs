//! Command line options and pattern expansion for `mqtt-bench`.
//!
//! The CLI intentionally mirrors `rmqtt-bench` (short flags, patterns such as
//! `{no}` / `{cid}` / `{random}`) so existing benchmark scripts keep working,
//! and adds a few extras that are needed for reproducible measurement:
//! `--duration`, `--max-inflight`, `--latency` accounting and `--json` export.

use std::fmt;

use anyhow::{anyhow, Result};
use bytes::Bytes;
use clap::{Args as ClapGroup, Parser, Subcommand};
use serde::Serialize;

/// MQTT protocol version used for the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// MQTT 3.1.1
    V3,
    /// MQTT 5.0
    V5,
}

impl Protocol {
    /// Wire protocol of the local MQTT codec.
    pub const fn version(self) -> crate::mqtt::Version {
        match self {
            Protocol::V3 => crate::mqtt::Version::V3,
            Protocol::V5 => crate::mqtt::Version::V5,
        }
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Protocol::V3 => write!(f, "3.1.1"),
            Protocol::V5 => write!(f, "5.0"),
        }
    }
}

/// MQTT benchmark tool, CLI compatible with rmqtt-bench.
#[derive(Debug, Parser)]
#[command(name = "mqtt-bench", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run benchmark with MQTT 3.1.1
    #[command(visible_alias = "311")]
    V3(Opts),
    /// Run benchmark with MQTT 5.0
    #[command(visible_alias = "50")]
    V5(Opts),
}

impl Cli {
    /// Resolve the parsed CLI into `(protocol, args)`.
    pub fn into_args(self) -> (Protocol, Opts) {
        match self.command {
            Command::V3(a) => (Protocol::V3, a),
            Command::V5(a) => (Protocol::V5, a),
        }
    }
}

/// Benchmark options shared by the `v3` and `v5` subcommands.
#[derive(Debug, Clone, ClapGroup, Serialize)]
pub struct Opts {
    /// MQTT broker endpoint list, e.g. --addrs 127.0.0.1:1883 127.0.0.1:1884
    #[arg(long, default_value = "127.0.0.1:1883")]
    pub addrs: Vec<String>,

    /// Local IP address list to bind outgoing sockets, e.g. --ifaddrs 127.0.0.1 127.0.0.2
    #[arg(long)]
    pub ifaddrs: Vec<String>,

    /// Number of connections (clients)
    #[arg(short = 'c', long, default_value_t = 1000)]
    pub conns: usize,

    /// Interval between two consecutive connections, milliseconds (0 = no delay)
    #[arg(short = 'i', long, default_value_t = 0)]
    pub conn_interval: u64,

    /// Stop the benchmark after N seconds (0 = run until Ctrl-C)
    #[arg(short = 'd', long, default_value_t = 0)]
    pub duration: u64,

    /// Client id pattern, supports {no} / {pid} / {random}
    #[arg(short = 'E', long, default_value = "bench-{pid}-{no}")]
    pub id_pattern: String,

    /// Username
    #[arg(short = 'u', long)]
    pub username: Option<String>,

    /// Password
    #[arg(short = 'p', long)]
    pub password: Option<String>,

    /// Handshake (connect + CONNACK) timeout, seconds
    #[arg(short = 'H', long, default_value_t = 30)]
    pub handshake_timeout: u64,

    /// Keepalive, seconds (0 disables keepalive)
    #[arg(short = 'k', long, default_value_t = 60)]
    pub keepalive: u16,

    /// Clean session / clean start
    #[arg(short = 'C', long, default_value_t = true, action = clap::ArgAction::Set)]
    pub clean: bool,

    /// Subscribe switch
    #[arg(short = 'S', long)]
    pub sub: bool,

    /// Publish switch
    #[arg(short = 'P', long)]
    pub pub_switch: bool,

    /// Subscription / publish topic pattern, supports {cid} / {no} / {random}
    #[arg(short = 't', long, default_value = "{cid}")]
    pub topic_pattern: String,

    /// QoS for both publishing and subscribing: 0, 1 or 2
    #[arg(short = 'q', long, default_value_t = 1)]
    pub qos: u8,

    /// Retain flag on published messages
    #[arg(short = 'r', long)]
    pub retain: bool,

    /// Auto reconnect interval, milliseconds (0 disables reconnecting)
    #[arg(short = 'a', long, default_value_t = 5000)]
    pub reconn_interval: u64,

    /// Publish interval in ms: 1 message per interval; 0 saturates the window
    #[arg(short = 'I', long, default_value_t = 1000)]
    pub pub_interval: u64,

    /// Publish payload size in bytes
    #[arg(short = 's', long, default_value_t = 256)]
    pub size: usize,

    /// Fixed payload content (overrides --size, latency stats unavailable)
    #[arg(short = 'm', long)]
    pub message: Option<String>,

    /// Total number of published messages per client, 0 = unlimited
    #[arg(short = 'l', long, default_value_t = 0)]
    pub max_limit: u64,

    /// Publish topic serial number range, e.g. -R 0 10000
    #[arg(short = 'R', long, num_args = 2, value_names = ["FROM", "TO"], default_values = ["0", "0"])]
    pub topic_no_range: Vec<usize>,

    /// Max un-acknowledged QoS 1/2 messages per client
    #[arg(long, default_value_t = 100)]
    pub max_inflight: usize,

    /// Consider a QoS 1/2 message lost after N seconds without ack (0 = never)
    #[arg(long, default_value_t = 30)]
    pub ack_timeout: u64,

    /// Keep receiving for N seconds after the run ends, so in-flight messages
    /// are drained and send/receive counts can be compared
    #[arg(long, default_value_t = 0)]
    pub drain: u64,

    /// Reason code sent in MQTT 5.0 PUBREL: 2 = Send Onward (the specification
    /// value, MQTT 5.0 section 3.4.4.1), 0 = the value oximqttd <= 0.22 accepts
    /// because its codec rejects 2 as a malformed packet.
    #[arg(long, default_value_t = 2)]
    pub v5_pubrel_reason: u8,

    /// Console stats output interval, seconds
    #[arg(short = 'o', long, default_value_t = 5)]
    pub output_interval: u64,

    /// Control switch: periodically disconnect & reconnect clients to simulate churn
    #[arg(short = 'T', long)]
    pub control: bool,

    /// Ratio of connections disconnected/reconnected per control interval
    #[arg(short = 'D', long, default_value_t = 0.4)]
    pub ctrl_disconn_ratio: f64,

    /// Control interval, milliseconds
    #[arg(short = 'L', long, default_value_t = 1000)]
    pub ctrl_interval: u64,

    /// Last will topic
    #[arg(long)]
    pub lw_topic: Option<String>,

    /// Last will message
    #[arg(long)]
    pub lw_msg: Option<String>,

    /// Last will QoS
    #[arg(long, default_value_t = 0)]
    pub lw_qos: u8,

    /// Last will retain flag
    #[arg(long)]
    pub lw_retain: bool,

    /// Write the final summary report to a JSON file
    #[arg(long)]
    pub json: Option<String>,
}

impl Default for Opts {
    /// Mirrors the clap defaults below, convenient for tests and tooling.
    fn default() -> Self {
        Opts {
            addrs: vec!["127.0.0.1:1883".to_owned()],
            ifaddrs: vec![],
            conns: 1000,
            conn_interval: 0,
            duration: 0,
            id_pattern: "bench-{pid}-{no}".to_owned(),
            username: None,
            password: None,
            handshake_timeout: 30,
            keepalive: 60,
            clean: true,
            sub: false,
            pub_switch: false,
            topic_pattern: "{cid}".to_owned(),
            qos: 1,
            retain: false,
            reconn_interval: 5000,
            pub_interval: 1000,
            size: 256,
            message: None,
            max_limit: 0,
            topic_no_range: vec![0, 0],
            max_inflight: 100,
            ack_timeout: 30,
            drain: 0,
            v5_pubrel_reason: 2,
            output_interval: 5,
            control: false,
            ctrl_disconn_ratio: 0.4,
            ctrl_interval: 1000,
            lw_topic: None,
            lw_msg: None,
            lw_qos: 0,
            lw_retain: false,
            json: None,
        }
    }
}

impl Opts {
    /// Validate the options and normalize derived values.
    pub fn validate(&mut self) -> Result<()> {
        if self.conns == 0 {
            return Err(anyhow!("-c/--conns must be greater than 0"));
        }
        if self.qos > 2 {
            return Err(anyhow!("-q/--qos must be 0, 1 or 2"));
        }
        if self.addrs.is_empty() {
            return Err(anyhow!("--addrs must not be empty"));
        }
        if self.size < LATENCY_HEADER_LEN && self.message.is_none() {
            return Err(anyhow!(
                "-s/--size must be at least {} bytes to carry the latency stamp",
                LATENCY_HEADER_LEN
            ));
        }
        if self.ctrl_disconn_ratio < 0.0 || self.ctrl_disconn_ratio > 1.0 {
            return Err(anyhow!("-D/--ctrl-disconn-ratio must be within 0.0..=1.0"));
        }
        if self.topic_no_range.len() != 2 {
            return Err(anyhow!(
                "-R/--topic-no-range takes exactly two values: FROM TO"
            ));
        }
        if self.topic_no_range[0] > self.topic_no_range[1] {
            return Err(anyhow!("-R FROM must be <= TO"));
        }
        if self.v5_pubrel_reason != 0 && self.v5_pubrel_reason != 2 {
            return Err(anyhow!("--v5-pubrel-reason must be 0 or 2 (Send Onward)"));
        }
        if self.max_inflight == 0 || self.max_inflight > 65535 {
            return Err(anyhow!("--max-inflight must be within 1..=65535"));
        }
        if self.output_interval == 0 {
            self.output_interval = 1;
        }
        if self.ctrl_interval == 0 {
            self.ctrl_interval = 1;
        }
        Ok(())
    }

    /// Inclusive publish topic serial range.
    pub fn topic_range(&self) -> (usize, usize) {
        (self.topic_no_range[0], self.topic_no_range[1])
    }

    /// Effective payload size (fixed message content wins over `--size`).
    pub fn payload_len(&self) -> usize {
        match &self.message {
            Some(m) => m.len(),
            None => self.size,
        }
    }

    /// Human readable summary printed once at startup.
    pub fn describe(&self, proto: Protocol) -> String {
        format!(
            "proto={} addrs={:?} conns={} interval={}ms duration={}s sub={} pub={} qos={} \
             topic={:?} range={:?}-{:?} size={}B keepalive={}s clean={} reconn={}ms \
             inflight={} control={}",
            proto,
            self.addrs,
            self.conns,
            self.conn_interval,
            self.duration,
            self.sub,
            self.pub_switch,
            self.qos,
            self.topic_pattern,
            self.topic_range().0,
            self.topic_range().1,
            self.payload_len(),
            self.keepalive,
            self.clean,
            self.reconn_interval,
            self.max_inflight,
            self.control,
        )
    }
}

/// Bytes reserved at the head of every payload for end-to-end latency accounting:
/// 2 magic bytes + 8 bytes big-endian microsecond timestamp.
pub const LATENCY_HEADER_LEN: usize = 10;

const LATENCY_MAGIC: [u8; 2] = [0xB5, 0x4C];

/// Tiny deterministic PRNG (xorshift64*) so each client can pick random topics
/// without the cost/contension of a shared RNG.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// Seed from a client serial number (never zero).
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6F_DD6D)
    }

    /// Uniform value in `range` (inclusive).
    #[inline]
    pub fn next_in(&mut self, range: (usize, usize)) -> usize {
        let span = range.1 - range.0 + 1;
        range.0 + (self.next_u64() as usize) % span
    }

    /// 8 hex chars, used by the `{random}` pattern.
    #[inline]
    pub fn token(&mut self) -> String {
        format!("{:08x}", (self.next_u64() >> 32) as u32)
    }
}

/// Current process id, used by the `{pid}` pattern token so that two
/// `mqtt-bench` processes never fight over the same client ids (which would
/// make the broker drop one side of each pair with a session takeover).
pub fn pid_token() -> &'static str {
    static PID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    PID.get_or_init(|| std::process::id().to_string())
}

/// Expand `{no}`, `{cid}`, `{pid}` and `{random}` inside an id/topic pattern.
///
/// Unknown braces are left untouched so literal topics can still be used.
pub fn expand(pattern: &str, no: usize, cid: &str, rnd: &mut Rng) -> String {
    if !pattern.contains('{') {
        return pattern.to_owned();
    }
    let mut out = String::with_capacity(pattern.len() + 16);
    let mut rest = pattern;
    while let Some(pos) = rest.find('{') {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos + 1..];
        match tail.find('}') {
            Some(end) => {
                match &tail[..end] {
                    "no" => out.push_str(&no.to_string()),
                    "cid" => out.push_str(cid),
                    "random" => out.push_str(&rnd.token()),
                    "pid" => out.push_str(pid_token()),
                    other => {
                        out.push('{');
                        out.push_str(other);
                        out.push('}');
                    }
                }
                rest = &tail[end + 1..];
            }
            None => {
                out.push('{');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Builds publish payloads. The first [`LATENCY_HEADER_LEN`] bytes carry a
/// magic marker plus a monotonic microsecond timestamp (when the payload is
/// large enough), which the receiving side turns into an end-to-end latency.
#[derive(Debug, Clone)]
pub struct PayloadBuilder {
    template: Vec<u8>,
    stamped: bool,
}

impl PayloadBuilder {
    /// Prepare the payload template from the resolved options.
    pub fn new(args: &Opts) -> Self {
        let mut template = match &args.message {
            Some(m) => m.clone().into_bytes(),
            None => vec![b'x'; args.size],
        };
        let stamped = template.len() >= LATENCY_HEADER_LEN;
        if stamped {
            template[..2].copy_from_slice(&LATENCY_MAGIC);
        }
        PayloadBuilder { template, stamped }
    }

    /// Encode `ts_us` into a fresh payload.
    pub fn build(&self, ts_us: u64) -> Bytes {
        if !self.stamped {
            return Bytes::from(self.template.clone());
        }
        let mut buf = self.template.clone();
        buf[2..LATENCY_HEADER_LEN].copy_from_slice(&ts_us.to_be_bytes());
        Bytes::from(buf)
    }

    /// Decode the timestamp of a received payload, `None` when not stamped.
    pub fn parse(payload: &[u8]) -> Option<u64> {
        if payload.len() < LATENCY_HEADER_LEN || payload[..2] != LATENCY_MAGIC {
            return None;
        }
        let mut raw = [0u8; 8];
        raw.copy_from_slice(&payload[2..LATENCY_HEADER_LEN]);
        let ts = u64::from_be_bytes(raw);
        // Reject garbage (a client id published as payload, all-0xff, ...)
        if ts == 0 || ts > 3_600_000_000_000 {
            None
        } else {
            Some(ts)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_all_tokens() {
        let mut rnd = Rng::new(1);
        assert_eq!(expand("iot/{no}", 42, "c-1", &mut rnd), "iot/42");
        assert_eq!(expand("{cid}/up", 7, "abc", &mut rnd), "abc/up");
        assert_eq!(expand("r-{random}", 1, "x", &mut rnd).len(), 10);
        assert_eq!(expand("plain/topic", 1, "x", &mut rnd), "plain/topic");
        assert_eq!(
            expand("p-{pid}", 1, "x", &mut rnd),
            format!("p-{}", std::process::id())
        );
        assert_eq!(expand("{unknown}/{no}", 3, "x", &mut rnd), "{unknown}/3");
        assert_eq!(expand("broken/{no", 3, "x", &mut rnd), "broken/{no");
    }

    #[test]
    fn rng_in_range() {
        let mut rnd = Rng::new(7);
        for _ in 0..1000 {
            let v = rnd.next_in((10, 12));
            assert!((10..=12).contains(&v));
        }
    }

    #[test]
    fn payload_roundtrip() {
        let args = Opts {
            addrs: vec!["a".into()],
            size: 64,
            ..Default::default()
        };
        let pb = PayloadBuilder::new(&args);
        let p = pb.build(1_234_567);
        assert_eq!(p.len(), 64);
        assert_eq!(PayloadBuilder::parse(&p), Some(1_234_567));

        // Non stamped payloads are ignored by the latency accounting.
        assert_eq!(PayloadBuilder::parse(b"hello world"), None);
        assert_eq!(PayloadBuilder::parse(&[0u8; 10]), None);
    }

    #[test]
    fn validate_rejects_bad_input() {
        let mut a = base_args();
        assert!(a.validate().is_ok());

        a.conns = 0;
        assert!(a.validate().is_err());

        let mut a = base_args();
        a.qos = 5;
        assert!(a.validate().is_err());

        let mut a = base_args();
        a.ctrl_disconn_ratio = 1.5;
        assert!(a.validate().is_err());

        let mut a = base_args();
        a.topic_no_range = vec![10, 1];
        assert!(a.validate().is_err());

        let mut a = base_args();
        a.size = 4;
        assert!(a.validate().is_err());
    }

    #[test]
    fn payload_len_prefers_message() {
        let mut a = base_args();
        assert_eq!(a.payload_len(), 256);
        a.message = Some("abc".into());
        assert_eq!(a.payload_len(), 3);
        // Too short for the stamp: the payload is sent verbatim.
        assert_eq!(
            PayloadBuilder::parse(&PayloadBuilder::new(&a).build(7)),
            None
        );
    }

    fn base_args() -> Opts {
        Opts::default()
    }
}
