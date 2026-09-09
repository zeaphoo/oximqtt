//! Lock-free benchmark statistics: counters, rates and a latency histogram.
//!
//! Every hot path uses relaxed atomics only, so a run with hundreds of
//! thousands of clients never contends on a mutex. Rates are derived by the
//! reporter (delta / elapsed) instead of being tracked per sample.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde::Serialize;

/// Histogram buckets: `0..63` are exact microseconds, above that 4 sub-buckets
/// per power-of-two octave, covering the full u64 range.
const BUCKETS: usize = 64 + 4 * 58;

/// Map a microsecond value to its histogram bucket.
///
/// Buckets are monotonic and the returned bucket's lower bound is always
/// `<= us`, which keeps percentile reports conservative (never optimistic).
#[inline]
fn bucket_of(us: u64) -> usize {
    if us < 64 {
        us as usize
    } else {
        let il = 64 - us.leading_zeros() as usize; // bit length, 7..=64
        let sub = ((us >> (il - 3)) & 0x3) as usize;
        (64 + (il - 7) * 4 + sub).min(BUCKETS - 1)
    }
}

/// Lower bound (µs) of a histogram bucket.
#[inline]
fn bucket_lower(idx: usize) -> u64 {
    if idx < 64 {
        idx as u64
    } else {
        let rel = idx - 64;
        let il = 7 + rel / 4;
        let sub = (rel % 4) as u64;
        (1u64 << (il - 1)) + (sub << (il - 3))
    }
}

#[inline]
fn inc(c: &AtomicU64, v: u64) {
    c.fetch_add(v, Ordering::Relaxed);
}

/// Raw counters of a running benchmark.
#[derive(Debug)]
pub struct Stats {
    conn_attempts: AtomicU64,
    conn_ok: AtomicU64,
    conn_fail: AtomicU64,
    active_conns: AtomicI64,
    subs: AtomicU64,
    sends: AtomicU64,
    send_bytes: AtomicU64,
    recvs: AtomicU64,
    recv_bytes: AtomicU64,
    pub_acks: AtomicU64,
    inflight: AtomicI64,
    inflight_full: AtomicU64,
    ack_timeouts: AtomicU64,
    closes: AtomicU64,
    reconnects: AtomicU64,
    errors: AtomicU64,
    pings: AtomicU64,
    dups: AtomicU64,
    recvs_retained: AtomicU64,
    latency_n: AtomicU64,
    latency_sum: AtomicU64,
    latency_max: AtomicU64,
    latency: Vec<AtomicU64>,
    last_err: Mutex<Option<String>>,
}

impl Default for Stats {
    fn default() -> Self {
        Self::new()
    }
}

impl Stats {
    /// New, empty statistics.
    pub fn new() -> Self {
        Stats {
            conn_attempts: AtomicU64::new(0),
            conn_ok: AtomicU64::new(0),
            conn_fail: AtomicU64::new(0),
            active_conns: AtomicI64::new(0),
            subs: AtomicU64::new(0),
            sends: AtomicU64::new(0),
            send_bytes: AtomicU64::new(0),
            recvs: AtomicU64::new(0),
            recv_bytes: AtomicU64::new(0),
            pub_acks: AtomicU64::new(0),
            inflight: AtomicI64::new(0),
            inflight_full: AtomicU64::new(0),
            ack_timeouts: AtomicU64::new(0),
            closes: AtomicU64::new(0),
            reconnects: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            pings: AtomicU64::new(0),
            dups: AtomicU64::new(0),
            recvs_retained: AtomicU64::new(0),
            latency_n: AtomicU64::new(0),
            latency_sum: AtomicU64::new(0),
            latency_max: AtomicU64::new(0),
            latency: (0..BUCKETS).map(|_| AtomicU64::new(0)).collect(),
            last_err: Mutex::new(None),
        }
    }

    /// Shared instance used by a real run, mirroring rmqtt-bench's
    /// `Stats::instance()`. Tests build their own via [`Stats::new`].
    pub fn global() -> Arc<Stats> {
        static STATS: OnceLock<Arc<Stats>> = OnceLock::new();
        STATS.get_or_init(|| Arc::new(Stats::new())).clone()
    }

    #[inline]
    fn lat(&self, idx: usize) -> &AtomicU64 {
        &self.latency[idx]
    }

    pub fn conn_attempt(&self) {
        inc(&self.conn_attempts, 1);
    }

    pub fn conn_established(&self) {
        inc(&self.conn_ok, 1);
        self.active_conns.fetch_add(1, Ordering::Relaxed);
    }

    pub fn conn_failed(&self, err: &str) {
        self.conn_failed_n(1, err);
    }

    pub fn conn_failed_n(&self, n: u64, err: &str) {
        inc(&self.conn_fail, n);
        inc(&self.errors, n);
        self.set_last_err(err);
    }

    pub fn conn_closed(&self) {
        inc(&self.closes, 1);
        self.active_conns.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn reconnected(&self) {
        inc(&self.reconnects, 1);
    }

    pub fn subscribed(&self) {
        inc(&self.subs, 1);
    }

    pub fn sent(&self, bytes: u64) {
        inc(&self.sends, 1);
        inc(&self.send_bytes, bytes);
    }

    pub fn received(&self, bytes: u64, retained: bool) {
        inc(&self.recvs, 1);
        inc(&self.recv_bytes, bytes);
        if retained {
            inc(&self.recvs_retained, 1);
        }
    }

    pub fn acked(&self) {
        inc(&self.pub_acks, 1);
    }

    pub fn inflight_inc(&self) {
        self.inflight.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inflight_dec(&self) {
        self.inflight.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn inflight_blocked(&self) {
        inc(&self.inflight_full, 1);
    }

    pub fn ack_timeout(&self) {
        inc(&self.ack_timeouts, 1);
        inc(&self.errors, 1);
    }

    pub fn pinged(&self) {
        inc(&self.pings, 1);
    }

    /// A QoS 2 publish we already processed was delivered again.
    pub fn duplicated(&self) {
        inc(&self.dups, 1);
    }

    pub fn error(&self, err: &str) {
        inc(&self.errors, 1);
        self.set_last_err(err);
    }

    fn set_last_err(&self, err: &str) {
        if let Ok(mut g) = self.last_err.lock() {
            *g = Some(err.to_owned());
        }
    }

    /// Read the last recorded error message without consuming it.
    pub fn peek_last_err(&self) -> Option<String> {
        self.last_err.lock().ok().and_then(|g| g.clone())
    }

    /// Take the last recorded error message (drains it).
    pub fn take_last_err(&self) -> Option<String> {
        self.last_err.lock().ok().and_then(|mut g| g.take())
    }

    /// Record an end-to-end latency sample in microseconds.
    #[inline]
    pub fn record_latency(&self, us: u64) {
        self.lat(bucket_of(us)).fetch_add(1, Ordering::Relaxed);
        inc(&self.latency_n, 1);
        inc(&self.latency_sum, us);
        self.latency_max.fetch_max(us, Ordering::Relaxed);
    }

    /// Percentile (0..=100) of recorded latencies, in microseconds.
    pub fn percentile(&self, p: f64) -> u64 {
        let total = self.latency_n.load(Ordering::Relaxed);
        if total == 0 {
            return 0;
        }
        let target = ((total as f64) * p / 100.0).ceil().max(1.0) as u64;
        let mut acc = 0u64;
        for (idx, c) in self.latency.iter().enumerate() {
            acc += c.load(Ordering::Relaxed);
            if acc >= target {
                return bucket_lower(idx);
            }
        }
        self.latency_max.load(Ordering::Relaxed)
    }

    /// Mean latency in microseconds.
    pub fn latency_mean(&self) -> u64 {
        let n = self.latency_n.load(Ordering::Relaxed);
        self.latency_sum
            .load(Ordering::Relaxed)
            .checked_div(n)
            .unwrap_or(0)
    }

    /// Current latency statistics.
    pub fn latency_stats(&self) -> Latency {
        Latency {
            samples: self.latency_n.load(Ordering::Relaxed),
            mean_us: self.latency_mean(),
            p50_us: self.percentile(50.0),
            p90_us: self.percentile(90.0),
            p99_us: self.percentile(99.0),
            p999_us: self.percentile(99.9),
            max_us: self.latency_max.load(Ordering::Relaxed),
        }
    }

    /// Snapshot of every counter, used by the reporter.
    pub fn snapshot(&self) -> Counters {
        Counters {
            conn_attempts: self.conn_attempts.load(Ordering::Relaxed),
            conn_ok: self.conn_ok.load(Ordering::Relaxed),
            conn_fail: self.conn_fail.load(Ordering::Relaxed),
            active_conns: self.active_conns.load(Ordering::Relaxed).max(0) as u64,
            subs: self.subs.load(Ordering::Relaxed),
            sends: self.sends.load(Ordering::Relaxed),
            send_bytes: self.send_bytes.load(Ordering::Relaxed),
            recvs: self.recvs.load(Ordering::Relaxed),
            recv_bytes: self.recv_bytes.load(Ordering::Relaxed),
            pub_acks: self.pub_acks.load(Ordering::Relaxed),
            inflight: self.inflight.load(Ordering::Relaxed).max(0) as u64,
            inflight_full: self.inflight_full.load(Ordering::Relaxed),
            ack_timeouts: self.ack_timeouts.load(Ordering::Relaxed),
            closes: self.closes.load(Ordering::Relaxed),
            reconnects: self.reconnects.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            pings: self.pings.load(Ordering::Relaxed),
            dups: self.dups.load(Ordering::Relaxed),
            recvs_retained: self.recvs_retained.load(Ordering::Relaxed),
        }
    }

    /// Build the final report from the current counters.
    pub fn report(&self, elapsed: Duration) -> Report {
        let secs = elapsed.as_secs_f64();
        let c = self.snapshot();
        Report {
            elapsed_secs: secs,
            counters: c,
            avg_rate: Rates {
                conn: rate(c.conn_ok, secs),
                sub: rate(c.subs, secs),
                send: rate(c.sends, secs),
                recv: rate(c.recvs, secs),
                send_mbps: (c.send_bytes as f64 * 8.0) / (secs * 1e6),
                recv_mbps: (c.recv_bytes as f64 * 8.0) / (secs * 1e6),
            },
            latency: self.latency_stats(),
            last_err: self.take_last_err(),
        }
    }
}

/// Immutable copy of all counters.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Counters {
    pub conn_attempts: u64,
    pub conn_ok: u64,
    pub conn_fail: u64,
    pub active_conns: u64,
    pub subs: u64,
    pub sends: u64,
    pub send_bytes: u64,
    pub recvs: u64,
    pub recv_bytes: u64,
    pub pub_acks: u64,
    pub inflight: u64,
    pub inflight_full: u64,
    pub ack_timeouts: u64,
    pub closes: u64,
    pub reconnects: u64,
    pub errors: u64,
    pub pings: u64,
    pub dups: u64,
    pub recvs_retained: u64,
}

impl Counters {
    /// Element-wise difference of two snapshots (`self - other`); gauges are
    /// copied instead of differenced.
    #[must_use]
    pub fn since(&self, other: &Counters) -> Counters {
        Counters {
            conn_attempts: self.conn_attempts - other.conn_attempts,
            conn_ok: self.conn_ok - other.conn_ok,
            conn_fail: self.conn_fail - other.conn_fail,
            active_conns: self.active_conns,
            subs: self.subs - other.subs,
            sends: self.sends - other.sends,
            send_bytes: self.send_bytes - other.send_bytes,
            recvs: self.recvs - other.recvs,
            recv_bytes: self.recv_bytes - other.recv_bytes,
            pub_acks: self.pub_acks - other.pub_acks,
            inflight: self.inflight,
            inflight_full: self.inflight_full - other.inflight_full,
            ack_timeouts: self.ack_timeouts - other.ack_timeouts,
            closes: self.closes - other.closes,
            reconnects: self.reconnects - other.reconnects,
            errors: self.errors - other.errors,
            pings: self.pings - other.pings,
            dups: self.dups - other.dups,
            recvs_retained: self.recvs_retained - other.recvs_retained,
        }
    }

    /// Rates of `self` (interpreted as a delta) over `dt`.
    pub fn rates(&self, dt: Duration) -> Rates {
        let secs = dt.as_secs_f64().max(0.001);
        Rates {
            conn: rate(self.conn_ok, secs),
            sub: rate(self.subs, secs),
            send: rate(self.sends, secs),
            recv: rate(self.recvs, secs),
            send_mbps: (self.send_bytes as f64 * 8.0) / (secs * 1e6),
            recv_mbps: (self.recv_bytes as f64 * 8.0) / (secs * 1e6),
        }
    }
}

/// Latency percentiles at report time.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Latency {
    pub samples: u64,
    pub mean_us: u64,
    pub p50_us: u64,
    pub p90_us: u64,
    pub p99_us: u64,
    pub p999_us: u64,
    pub max_us: u64,
}

/// Full machine readable report.
#[derive(Debug, Serialize)]
pub struct Report {
    pub elapsed_secs: f64,
    pub counters: Counters,
    pub avg_rate: Rates,
    pub latency: Latency,
    pub last_err: Option<String>,
}

/// Message / connection rates per second.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Rates {
    pub conn: f64,
    pub sub: f64,
    pub send: f64,
    pub recv: f64,
    pub send_mbps: f64,
    pub recv_mbps: f64,
}

#[inline]
fn rate(v: u64, secs: f64) -> f64 {
    // A zero elapsed time (sub-millisecond runs) would divide by zero.
    (v as f64) / secs.max(1e-9)
}

/// Formats the periodic one-line console stats, similar to rmqtt-bench.
pub fn format_line(
    elapsed: Duration,
    totals: &Counters,
    delta: &Counters,
    dt: Duration,
    lat: &Latency,
) -> String {
    let r = delta.rates(dt);
    format!(
        "[{:>6.1}s] conns:{} ({:0.0}/s, fail:{}) subs:{} ({:0.0}/s) sends:{} ({:0.0}/s) \
         recvs:{} ({:0.0}/s) inflight:{} lat p50:{:?} p99:{:?} max:{:?}",
        elapsed.as_secs_f64(),
        totals.active_conns,
        r.conn,
        totals.conn_fail,
        totals.subs,
        r.sub,
        totals.sends,
        r.send,
        totals.recvs,
        r.recv,
        totals.inflight,
        Duration::from_micros(lat.p50_us),
        Duration::from_micros(lat.p99_us),
        Duration::from_micros(lat.max_us),
    )
}

/// Formats a multi-line human readable final summary.
pub fn format_summary(r: &Report) -> String {
    let c = &r.counters;
    let dur = Duration::from_micros;
    let mut s = String::new();
    s.push_str("==================== SUMMARY ====================\n");
    s.push_str(&format!("elapsed            : {:.2}s\n", r.elapsed_secs));
    s.push_str(&format!(
        "connections        : {} ok, {} fail, {} active, {} reconnects\n",
        c.conn_ok, c.conn_fail, c.active_conns, c.reconnects
    ));
    s.push_str(&format!("connect rate       : {:.0}/s\n", r.avg_rate.conn));
    s.push_str(&format!(
        "subscriptions      : {} ({:.0}/s)\n",
        c.subs, r.avg_rate.sub
    ));
    s.push_str(&format!(
        "published          : {} msgs, {:.2} MB, {:.0} msg/s, {:.2} Mbps\n",
        c.sends,
        c.send_bytes as f64 / 1048576.0,
        r.avg_rate.send,
        r.avg_rate.send_mbps
    ));
    s.push_str(&format!(
        "received           : {} msgs, {:.2} MB, {:.0} msg/s, {:.2} Mbps\n",
        c.recvs,
        c.recv_bytes as f64 / 1048576.0,
        r.avg_rate.recv,
        r.avg_rate.recv_mbps
    ));
    s.push_str(&format!(
        "qos                : {} acks, {} inflight, {} blocked, {} ack-timeouts\n",
        c.pub_acks, c.inflight, c.inflight_full, c.ack_timeouts
    ));
    if r.latency.samples > 0 {
        s.push_str(&format!(
            "e2e latency        : n={} mean={:?} p50={:?} p90={:?} p99={:?} p99.9={:?} max={:?}\n",
            r.latency.samples,
            dur(r.latency.mean_us),
            dur(r.latency.p50_us),
            dur(r.latency.p90_us),
            dur(r.latency.p99_us),
            dur(r.latency.p999_us),
            dur(r.latency.max_us)
        ));
    } else {
        s.push_str("e2e latency        : no samples (stamped payload required)\n");
    }
    s.push_str(&format!("errors             : {}\n", c.errors));
    if let Some(err) = &r.last_err {
        s.push_str(&format!("last error         : {}\n", err));
    }
    s.push_str("=================================================");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_exact_in_linear_region() {
        for us in 0..64u64 {
            assert_eq!(bucket_lower(bucket_of(us)), us, "us={}", us);
        }
    }

    #[test]
    fn bucket_monotonic_and_bounded() {
        let mut prev_idx = 0usize;
        let mut prev_lower = 0u64;
        for us in [
            64u64,
            65,
            95,
            100,
            127,
            128,
            1000,
            999_999,
            1_000_000,
            1_000_000_000,
            4_000_000_000,
        ] {
            let idx = bucket_of(us);
            assert!(idx < BUCKETS, "bucket {} out of range for {}", idx, us);
            assert!(idx >= prev_idx, "bucket went backwards at {}", us);
            let lower = bucket_lower(idx);
            assert!(lower <= us, "lower bound {} exceeds sample {}", lower, us);
            assert!(lower >= prev_lower, "lower bound went backwards at {}", us);
            prev_idx = idx;
            prev_lower = lower;
        }
    }

    #[test]
    fn percentile_matches_input() {
        let s = Stats::new();
        for us in 1..=100u64 {
            s.record_latency(us);
        }
        assert_eq!(s.latency_n.load(Ordering::Relaxed), 100);
        assert!(
            (49..=51).contains(&s.percentile(50.0)),
            "p50={}",
            s.percentile(50.0)
        );
        assert!(
            (96..=100).contains(&s.percentile(99.0)),
            "p99={}",
            s.percentile(99.0)
        );
        // p100 is bucket-granular above 63us: the last bucket starts at 96.
        assert!(s.percentile(100.0) >= 96 && s.percentile(100.0) <= 100);
        assert_eq!(s.latency_mean(), 50);
        assert_eq!(s.latency_stats().max_us, 100);
    }

    #[test]
    fn percentile_of_empty_histogram_is_zero() {
        let s = Stats::new();
        assert_eq!(s.percentile(50.0), 0);
        assert_eq!(s.latency_mean(), 0);
    }

    #[test]
    fn counters_and_rates() {
        let s = Stats::new();
        s.conn_attempt();
        s.conn_established();
        s.conn_established();
        s.conn_closed();
        s.sent(1000);
        s.received(500, false);
        s.received(64, true);
        assert_eq!(s.active_conns.load(Ordering::Relaxed), 1);

        let c = s.snapshot();
        assert_eq!(
            (c.conn_attempts, c.conn_ok, c.conn_fail, c.sends, c.recvs),
            (1, 2, 0, 1, 2)
        );
        assert_eq!(c.recvs_retained, 1);

        let r = s.report(Duration::from_secs(2));
        assert_eq!(r.avg_rate.conn, 1.0);
        assert_eq!(r.avg_rate.send, 0.5);
        assert_eq!(r.avg_rate.send_mbps, 0.004);
        assert_eq!(r.avg_rate.recv, 1.0);

        s.conn_failed("boom");
        assert_eq!(s.take_last_err().as_deref(), Some("boom"));
        assert!(s.take_last_err().is_none());
    }

    #[test]
    fn counters_since_subtracts_monotons_and_keeps_gauges() {
        let prev = Counters {
            conn_ok: 4,
            sends: 30,
            subs: 2,
            active_conns: 3,
            ..Default::default()
        };
        let now = Counters {
            conn_ok: 10,
            sends: 100,
            subs: 5,
            active_conns: 7,
            dups: 1,
            pings: 3,
            ..Default::default()
        };
        let d = now.since(&prev);
        assert_eq!((d.conn_ok, d.sends, d.subs), (6, 70, 3));
        assert_eq!(d.active_conns, 7, "gauge is copied, not differenced");
        assert_eq!((d.dups, d.pings), (1, 3));
    }

    #[test]
    fn rates_use_delta_counters() {
        let d = Counters {
            sends: 500,
            recvs: 250,
            conn_ok: 10,
            ..Default::default()
        };
        let r = d.rates(Duration::from_secs(5));
        assert_eq!(r.send, 100.0);
        assert_eq!(r.recv, 50.0);
        assert_eq!(r.conn, 2.0);
    }

    #[test]
    fn formatters_do_not_panic() {
        let s = Stats::new();
        s.conn_established();
        s.record_latency(1234);
        let snap = s.snapshot();
        let line = format_line(
            Duration::from_secs(5),
            &snap,
            &snap,
            Duration::from_secs(1),
            &s.latency_stats(),
        );
        assert!(line.contains("conns:1"), "{}", line);
        assert!(line.contains("sends:0"));

        let rep = s.report(Duration::from_secs(3));
        let sum = format_summary(&rep);
        assert!(sum.contains("SUMMARY"));
        assert!(sum.contains("published"));
        assert!(sum.contains("p50"));

        let js = serde_json::to_string(&rep).unwrap();
        assert!(js.contains("\"conn_ok\""));
        assert!(js.contains("\"p99_us\""));
    }

    #[test]
    fn summary_without_latency() {
        let s = Stats::new();
        let sum = format_summary(&s.report(Duration::from_secs(1)));
        assert!(sum.contains("no samples"));
    }
}
