//! The benchmark client: one tokio task per MQTT connection.
//!
//! A client runs `connect -> subscribe -> publish/receive -> ...` and
//! reconnects on errors, mirroring rmqtt-bench semantics. Everything a
//! connection owns (socket halves, packet id slots, inflight map) lives on the
//! task itself, so there is no channel, no mutex and no shared state beyond
//! the global counters.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use tokio::time;

use crate::mqtt::{Packet, QoS};
use crate::options::{expand, Opts, PayloadBuilder, Protocol, Rng};
use crate::packets::{self, Incoming};
use crate::stats::Stats;
use crate::transport::{self, Reader, Writer};
use crate::window::Slots;

/// Everything shared by the client tasks of one run.
#[derive(Debug)]
pub struct Ctx {
    pub args: Opts,
    pub proto: Protocol,
    pub peers: Vec<SocketAddr>,
    pub binds: Vec<IpAddr>,
    pub payload: PayloadBuilder,
    /// Monotonic reference used to stamp payloads (microseconds since start).
    pub epoch: Instant,
    pub stop: Arc<AtomicBool>,
    /// Per-client "kick" flags used by the churn (`-T`) controller.
    pub kicks: Vec<AtomicBool>,
    pub stats: Arc<Stats>,
}

impl Ctx {
    /// Resolve broker endpoints and local bind addresses of a real run.
    pub fn new(opts: Opts, proto: Protocol, stop: Arc<AtomicBool>) -> Result<Self> {
        Self::build(opts, proto, stop, Stats::global())
    }

    /// Constructor for unit tests: gets a private [`Stats`] instance so that
    /// tests running in parallel cannot observe each other's counters.
    #[cfg(test)]
    pub fn for_test(opts: Opts, proto: Protocol) -> Self {
        Self::build(
            opts,
            proto,
            Arc::new(AtomicBool::new(false)),
            Arc::new(Stats::new()),
        )
        .expect("test ctx needs resolvable loopback endpoints")
    }

    fn build(
        args: Opts,
        proto: Protocol,
        stop: Arc<AtomicBool>,
        stats: Arc<Stats>,
    ) -> Result<Self> {
        let mut peers = Vec::new();
        for addr in &args.addrs {
            let resolved: Vec<SocketAddr> = addr
                .to_socket_addrs()
                .map_err(|e| anyhow!("cannot resolve broker address {addr}: {e}"))?
                .collect();
            if resolved.is_empty() {
                return Err(anyhow!("no address resolved for {addr}"));
            }
            peers.extend(resolved);
        }
        let mut binds = Vec::new();
        for ifaddr in &args.ifaddrs {
            binds.push(
                ifaddr
                    .parse::<IpAddr>()
                    .map_err(|e| anyhow!("--ifaddrs value {ifaddr:?} is not an IP: {e}"))?,
            );
        }
        Ok(Ctx {
            payload: PayloadBuilder::new(&args),
            epoch: Instant::now(),
            kicks: (0..args.conns).map(|_| AtomicBool::new(false)).collect(),
            args,
            proto,
            peers,
            binds,
            stop,
            stats,
        })
    }

    #[inline]
    pub fn now_us(&self) -> u64 {
        self.epoch.elapsed().as_micros() as u64
    }

    /// Whether the whole run has been asked to stop.
    #[inline]
    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    /// Ask every task (clients, reporter, churn) to stop.
    #[inline]
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// QoS level of the run.
    #[inline]
    pub fn qos(&self) -> QoS {
        QoS::from_u8(self.args.qos)
    }

    /// Client id of connection `no`.
    pub fn client_id(&self, no: usize) -> String {
        let mut rnd = Rng::new(no as u64 + 1);
        expand(&self.args.id_pattern, no, &format!("bench-{no}"), &mut rnd)
    }

    /// Broker address and local source address of connection `no`.
    pub fn endpoints(&self, no: usize) -> (SocketAddr, Option<IpAddr>) {
        let peer = self.peers[no % self.peers.len()];
        let bind = if self.binds.is_empty() {
            None
        } else {
            Some(self.binds[no % self.binds.len()])
        };
        (peer, bind)
    }

    /// Ask connection `no` to drop and reconnect (churn controller).
    pub fn kick(&self, no: usize) {
        self.kicks[no].store(true, Ordering::Relaxed);
    }

    fn take_kick(&self, no: usize) -> bool {
        self.kicks[no].swap(false, Ordering::Relaxed)
    }
}

/// How one session ended.
enum SessionEnd {
    /// Keep the slot, reconnect after `--reconn-interval`.
    Reconnect,
    /// The client published everything it was asked to publish.
    Finished,
}

/// Client task body: keeps one logical connection alive across reconnects.
pub async fn run(ctx: Arc<Ctx>, no: usize) {
    let id = ctx.client_id(no);
    loop {
        if ctx.stopped() {
            break;
        }
        match session(&ctx, no, &id).await {
            Ok(SessionEnd::Finished) => break,
            Ok(SessionEnd::Reconnect) => {}
            Err(err) => {
                ctx.stats.conn_failed(&err.to_string());
                tracing::debug!(client = %id, "session error: {err:#}");
            }
        }
        if ctx.stopped() || ctx.args.reconn_interval == 0 {
            break;
        }
        ctx.stats.reconnected();
        time::sleep(Duration::from_millis(ctx.args.reconn_interval)).await;
    }
}

/// One CONNECT .. DISCONNECT cycle of a single client.
async fn session(ctx: &Arc<Ctx>, no: usize, id: &str) -> Result<SessionEnd> {
    let (peer, bind) = ctx.endpoints(no);
    let handshake = Duration::from_secs(ctx.args.handshake_timeout.max(1));

    ctx.stats.conn_attempt();
    let (mut rd, mut wr) =
        match transport::connect(peer, bind, handshake, ctx.proto.version()).await {
            Ok(halves) => halves,
            Err(e) => return Err(anyhow!("tcp connect to {peer} failed: {e}")),
        };

    let outcome = establish_and_run(ctx, no, id, &mut rd, &mut wr, peer, handshake).await;

    let _ = wr.send(&packets::disconnect()).await;
    let _ = wr.flush().await;
    rd.close();
    wr.close().await;
    ctx.stats.conn_closed();
    outcome
}

async fn establish_and_run(
    ctx: &Arc<Ctx>,
    no: usize,
    id: &str,
    rd: &mut Reader,
    wr: &mut Writer,
    peer: SocketAddr,
    handshake: Duration,
) -> Result<SessionEnd> {
    wr.send(&packets::connect(ctx, id)).await?;
    wr.flush().await?;
    let receive_max = time::timeout(handshake, wait_for(ctx, rd, wr, Want::ConnAck))
        .await
        .map_err(|_| anyhow!("CONNACK timeout from {peer}"))??;
    ctx.stats.conn_established();

    if ctx.args.sub {
        let mut rnd = Rng::new(no as u64 + 1);
        let filter = expand(&ctx.args.topic_pattern, no, id, &mut rnd);
        wr.send(&packets::subscribe(ctx, &filter, 1)).await?;
        wr.flush().await?;
        time::timeout(handshake, wait_for(ctx, rd, wr, Want::SubAck))
            .await
            .map_err(|_| anyhow!("SUBACK timeout from {peer}"))??;
        ctx.stats.subscribed();
    }
    traffic(ctx, no, id, rd, wr, receive_max).await
}

/// What the setup phase is waiting for.
enum Want {
    ConnAck,
    SubAck,
}

/// Read until the expected control packet arrives. Publishes that show up
/// early are counted and acknowledged so the broker does not redeliver them.
///
/// Returns the broker's advertised receive maximum on the CONNACK phase so the
/// publish window can honour it, and `None` once a subscription is confirmed.
async fn wait_for(
    ctx: &Arc<Ctx>,
    rd: &mut Reader,
    wr: &mut Writer,
    want: Want,
) -> Result<Option<u16>> {
    loop {
        match packets::classify(&rd.read_packet().await?) {
            Incoming::ConnAck {
                accepted,
                reason,
                receive_max,
            } => {
                if matches!(want, Want::ConnAck) {
                    return if accepted {
                        Ok(receive_max)
                    } else {
                        Err(anyhow!("connection refused: {reason}"))
                    };
                }
            }
            Incoming::SubAck { granted } => {
                if matches!(want, Want::SubAck) {
                    return match granted.iter().position(|g| g.is_none()) {
                        Some(_) => Err(anyhow!("subscription refused")),
                        None => Ok(None),
                    };
                }
            }
            Incoming::Publish {
                qos,
                packet_id,
                payload,
                retain,
                ..
            } => {
                ctx.stats.received(payload.len() as u64, retain);
                if let Some(ts) = PayloadBuilder::parse(&payload) {
                    ctx.stats.record_latency(ctx.now_us().saturating_sub(ts));
                }
                if let Some((kind, pid)) = packets::reply_for(qos, packet_id) {
                    wr.send(&packets::ack(ctx, kind, pid)).await?;
                    wr.flush().await?;
                }
            }
            Incoming::ServerDisconnect(reason) => {
                return Err(anyhow!("server disconnected during setup: {reason}"))
            }
            _ => {}
        }
    }
}

/// Steady state: publish on a timer, receive continuously, ack everything.
async fn traffic(
    ctx: &Arc<Ctx>,
    no: usize,
    id: &str,
    rd: &mut Reader,
    wr: &mut Writer,
    receive_max: Option<u16>,
) -> Result<SessionEnd> {
    let args = &ctx.args;
    let mut out = Outbound::new(ctx, no, receive_max);
    // QoS 2 inbound handshakes that still owe us a PUBCOMP.
    let mut awaiting_rel: Vec<u16> = Vec::new();
    let mut pending: Vec<Packet> = Vec::with_capacity(args.max_inflight + 8);
    let mut pub_done = false;

    let mut pub_tick = time::interval(Duration::from_millis(args.pub_interval.max(1)));
    pub_tick.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
    let keepalive = if args.keepalive > 0 {
        Duration::from_secs((args.keepalive as u64 / 2).max(1))
    } else {
        Duration::MAX / 4
    };
    let mut ping_tick = time::interval(keepalive);
    ping_tick.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
    let mut sweep_tick = time::interval(Duration::from_secs(1));
    sweep_tick.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
    let mut last_write = Instant::now();
    // Pinned so the 50ms polling interval survives across `select!` iterations;
    // re-creating the future every round would restart the sleep forever.
    let stopped = wait_stopped(ctx.stop.clone());
    tokio::pin!(stopped);

    loop {
        pending.clear();
        tokio::select! {
            biased;

            incoming = rd.read_packet() => {
                match incoming {
                    Ok(pkt) => on_incoming(ctx, packets::classify(&pkt), &mut out, &mut awaiting_rel, &mut pending)?,
                    Err(e) => {
                        let _ = wr.flush().await;
                        let hint = if ctx.proto == Protocol::V5 && out.unreleased_pubrel() > 0 {
                            " (session died with PUBREL unconfirmed: this broker rejects the                              spec PUBREL reason code, retry with --v5-pubrel-reason 0)"
                        } else {
                            ""
                        };
                        return Err(anyhow!("read error on {id}: {e}{hint}"));
                    }
                }
            }

            _ = pub_tick.tick(), if args.pub_switch && !pub_done => {
                out.fill(ctx, id, &mut pending);
                pub_done = out.limit_reached(ctx);
                last_write = Instant::now();
                if pub_done && !args.sub {
                    flush(wr, &mut pending).await?;
                    return Ok(SessionEnd::Finished);
                }
            }

            _ = ping_tick.tick() => {
                if last_write.elapsed() >= keepalive {
                    pending.push(packets::ping_req());
                    ctx.stats.pinged();
                    last_write = Instant::now();
                }
            }

            _ = sweep_tick.tick() => {
                out.sweep_timeouts(ctx, ctx.now_us());
            }

            _ = &mut stopped => {
                flush(wr, &mut pending).await?;
                if args.drain > 0 {
                    drain(ctx, no, id, rd, wr, &mut out, &mut awaiting_rel, &mut pending).await?;
                }
                return Ok(SessionEnd::Reconnect);
            }
        }

        flush(wr, &mut pending).await?;
        if out.stuck() > 0 {
            tracing::trace!(client = %id, stuck = out.stuck(), "unacknowledged messages");
        }
        if ctx.take_kick(no) {
            tracing::debug!(client = %id, "dropped by churn controller");
            return Ok(SessionEnd::Reconnect);
        }
    }
}

async fn flush(wr: &mut Writer, pending: &mut Vec<Packet>) -> Result<()> {
    for pkt in pending.drain(..) {
        wr.send(&pkt).await?;
    }
    wr.flush().await.map_err(Into::into)
}

/// Handle an inbound packet in steady state, queueing protocol replies.
fn on_incoming(
    ctx: &Arc<Ctx>,
    pkt: Incoming,
    out: &mut Outbound,
    awaiting_rel: &mut Vec<u16>,
    pending: &mut Vec<Packet>,
) -> Result<()> {
    match pkt {
        Incoming::Publish {
            topic,
            qos,
            packet_id,
            payload,
            retain,
        } => {
            tracing::trace!(topic, qos, retain, "publish received");
            ctx.stats.received(payload.len() as u64, retain);
            if let Some(ts) = PayloadBuilder::parse(&payload) {
                ctx.stats.record_latency(ctx.now_us().saturating_sub(ts));
            }
            match (qos, packet_id) {
                (1, Some(pid)) => pending.push(packets::ack(ctx, packets::Ack::PubAck, pid)),
                (2, Some(pid)) => {
                    if awaiting_rel.contains(&pid) {
                        // Redelivery of a message we already processed.
                        ctx.stats.duplicated();
                    } else {
                        awaiting_rel.push(pid);
                    }
                    pending.push(packets::ack(ctx, packets::Ack::PubRec, pid));
                }
                _ => {}
            }
        }
        Incoming::PubRel { pid } => {
            if let Some(pos) = awaiting_rel.iter().position(|p| *p == pid) {
                awaiting_rel.swap_remove(pos);
            }
            pending.push(packets::ack(ctx, packets::Ack::PubComp, pid));
        }
        Incoming::PubAck { pid } => out.on_ack(ctx, pid),
        Incoming::PubComp { pid } => out.on_ack(ctx, pid),
        Incoming::PubRec { pid } => out.on_pubrec(ctx, pid, pending),
        Incoming::ServerDisconnect(reason) => {
            return Err(anyhow!("server sent DISCONNECT: {reason}"))
        }
        Incoming::ConnAck {
            accepted: false,
            reason,
            ..
        } => {
            return Err(anyhow!("connection dropped: {reason}"));
        }
        _ => {}
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "the session state is passed as a group"
)]
/// After the run stops, keep reading until the inbound flow goes quiet, bounded
/// by `--drain`. Idle-based rather than a plain deadline so the tail of a burst
/// is fully accounted for without making every run longer.
async fn drain(
    ctx: &Arc<Ctx>,
    _no: usize,
    id: &str,
    rd: &mut Reader,
    wr: &mut Writer,
    out: &mut Outbound,
    awaiting_rel: &mut Vec<u16>,
    pending: &mut Vec<Packet>,
) -> Result<()> {
    /// Give up waiting for the next message after this long.
    const QUIET: Duration = Duration::from_millis(300);
    /// Absolute ceiling, so a stuck connection can never hang the shutdown.
    const CAP: Duration = Duration::from_secs(10);

    let window = Duration::from_secs(ctx.args.drain);
    let started = Instant::now();
    let mut last_msg = Instant::now();
    tracing::debug!(client = %id, "draining in-flight messages");

    while started.elapsed() < window + CAP {
        match time::timeout(QUIET, rd.read_packet()).await {
            Ok(Ok(pkt)) => {
                last_msg = Instant::now();
                on_incoming(ctx, packets::classify(&pkt), out, awaiting_rel, pending)?;
                flush(wr, pending).await?;
            }
            // The peer is gone, nothing else will arrive.
            Ok(Err(_)) => break,
            Err(_) => {}
        }
        if started.elapsed() >= window && last_msg.elapsed() >= QUIET {
            break;
        }
    }
    flush(wr, pending).await
}

/// Waits until the run-wide stop flag is set; used as a `select!` branch.
async fn wait_stopped(flag: Arc<AtomicBool>) {
    loop {
        if flag.load(Ordering::Relaxed) {
            return;
        }
        time::sleep(Duration::from_millis(50)).await;
    }
}

/// Publish side of one connection: packet id window + inflight tracking.
struct Outbound {
    qos: u8,
    slots: Slots,
    inflight: HashMap<u16, InflightMsg>,
    sent: u64,
    rnd: Rng,
}

/// An un-acknowledged publish: timestamp for stall detection, plus whether we
/// already sent PUBREL (used to explain a v5 QoS 2 disconnect).
struct InflightMsg {
    ts_us: u64,
    pubrel_sent: bool,
}

impl Outbound {
    fn new(ctx: &Arc<Ctx>, no: usize, receive_max: Option<u16>) -> Self {
        // Never send more un-acknowledged QoS 1/2 messages than the broker
        // advertised in its CONNACK (v5 Receive Maximum); v3 has no such
        // field, so only the local --max-inflight window applies there.
        let window = match receive_max {
            Some(broker) => ctx.args.max_inflight.min(broker as usize).max(1),
            None => ctx.args.max_inflight,
        };
        Outbound {
            qos: ctx.args.qos,
            slots: Slots::new(window),
            inflight: HashMap::new(),
            sent: 0,
            rnd: Rng::new(no as u64 + 1),
        }
    }

    #[inline]
    fn limit_reached(&self, ctx: &Arc<Ctx>) -> bool {
        ctx.args.max_limit > 0 && self.sent >= ctx.args.max_limit
    }

    /// Queue as many publishes as the window (QoS 1/2) or the batch size
    /// (QoS 0) allows.
    fn fill(&mut self, ctx: &Arc<Ctx>, id: &str, pending: &mut Vec<Packet>) {
        let range = ctx.args.topic_range();
        // `--pub-interval` paces one message per tick; `-I 0` saturates the
        // connection by filling the whole inflight window (which is also the
        // natural backpressure for QoS 1/2).
        let batch = if ctx.args.pub_interval == 0 {
            ctx.args.max_inflight
        } else {
            1
        };
        let mut queued = 0;
        while queued < batch {
            if self.limit_reached(ctx) {
                break;
            }
            let pid = if self.qos > 0 {
                match self.slots.take() {
                    Some(pid) => pid,
                    None => {
                        ctx.stats.inflight_blocked();
                        break;
                    }
                }
            } else {
                0
            };
            let topic = expand(
                &ctx.args.topic_pattern,
                self.rnd.next_in(range),
                id,
                &mut self.rnd,
            );
            let ts = ctx.now_us();
            let payload = ctx.payload.build(ts);
            let wire =
                packets::publish_wire_size(ctx.proto, &topic, payload.len(), self.qos) as u64;
            if self.qos > 0 {
                self.inflight.insert(
                    pid,
                    InflightMsg {
                        ts_us: ts,
                        pubrel_sent: false,
                    },
                );
                ctx.stats.inflight_inc();
            }
            pending.push(packets::publish(
                ctx,
                &topic,
                payload,
                (self.qos > 0).then_some(pid),
            ));
            ctx.stats.sent(wire);
            self.sent += 1;
            queued += 1;
        }
    }

    /// PUBACK / PUBCOMP completed a message.
    fn on_ack(&mut self, ctx: &Arc<Ctx>, pid: u16) {
        if self.inflight.remove(&pid).is_some() {
            self.slots.release(pid);
            ctx.stats.inflight_dec();
            ctx.stats.acked();
        } else {
            ctx.stats.error(&format!("ack for unknown packet id {pid}"));
        }
    }

    /// Messages still unacknowledged when the session ends.
    fn stuck(&self) -> usize {
        self.slots.in_use()
    }

    /// PUBREC: reply PUBREL, the message stays inflight until PUBCOMP.
    fn on_pubrec(&mut self, ctx: &Arc<Ctx>, pid: u16, pending: &mut Vec<Packet>) {
        if let Some(msg) = self.inflight.get_mut(&pid) {
            msg.pubrel_sent = true;
            pending.push(packets::ack(ctx, packets::Ack::PubRel, pid));
        }
    }

    /// Messages we released (PUBREL sent) that the broker never completed.
    fn unreleased_pubrel(&self) -> usize {
        self.inflight.values().filter(|m| m.pubrel_sent).count()
    }

    /// Drop messages that never got acknowledged. `now_us` is passed in so the
    /// bookkeeping stays testable without sleeping.
    fn sweep_timeouts(&mut self, ctx: &Arc<Ctx>, now_us: u64) {
        if ctx.args.ack_timeout == 0 {
            return;
        }
        let cutoff = now_us.saturating_sub(ctx.args.ack_timeout * 1_000_000);
        let stale: Vec<u16> = self
            .inflight
            .iter()
            .filter(|(_, m)| m.ts_us < cutoff)
            .map(|(pid, _)| *pid)
            .collect();
        for pid in stale {
            self.inflight.remove(&pid);
            self.slots.release(pid);
            ctx.stats.inflight_dec();
            ctx.stats.ack_timeout();
        }
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
