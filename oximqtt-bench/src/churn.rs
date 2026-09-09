//! Run control: periodic connection churn, the `--duration` timer and the
//! Ctrl-C handler.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::time;

use crate::client::Ctx;
use crate::options::Rng;

/// Churn controller: disconnects `--ctrl-disconn-ratio` of the connections
/// every `--ctrl-interval` milliseconds, which keeps reconnecting them.
pub async fn controller(ctx: Arc<Ctx>) {
    let mut rnd = Rng::new(0xC0FF_EE01);
    let every = Duration::from_millis(ctx.args.ctrl_interval);
    let last = ctx.args.conns - 1;
    let hits = ((ctx.args.conns as f64) * ctx.args.ctrl_disconn_ratio).ceil() as usize;
    loop {
        time::sleep(every).await;
        if ctx.stopped() {
            break;
        }
        for _ in 0..hits {
            ctx.kick(rnd.next_in((0, last)));
        }
        tracing::debug!("churn: {hits} connections scheduled for reconnect");
    }
}

/// Sets the stop flag once `--duration` seconds have elapsed.
pub async fn scheduled_stop(ctx: Arc<Ctx>, secs: u64) {
    time::sleep(Duration::from_secs(secs)).await;
    if !ctx.stopped() {
        eprintln!("  {}s duration reached, shutting down", secs);
    }
    ctx.stop();
}

/// Ctrl-C ends the run gracefully.
pub async fn signal_stop(stop: Arc<AtomicBool>) {
    if tokio::signal::ctrl_c().await.is_ok() {
        eprintln!("  interrupted, shutting down");
        stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::{Opts, Protocol};

    fn ctx(conns: usize, ratio: f64) -> Arc<Ctx> {
        let args = Opts {
            conns,
            control: true,
            ctrl_disconn_ratio: ratio,
            ctrl_interval: 10,
            ..Default::default()
        };
        Arc::new(Ctx::for_test(args, Protocol::V3))
    }

    #[tokio::test]
    async fn scheduled_stop_sets_the_flag() {
        let c = ctx(2, 0.5);
        assert!(!c.stopped());
        tokio::time::timeout(Duration::from_secs(3), scheduled_stop(c.clone(), 0))
            .await
            .unwrap();
        assert!(c.stopped());
    }

    #[tokio::test]
    async fn churn_kicks_clients_until_stopped() {
        let c = ctx(10, 0.3);
        let handle = tokio::spawn(controller(c.clone()));
        tokio::time::sleep(Duration::from_millis(80)).await;
        c.stop();
        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .unwrap()
            .unwrap();
        // With a 30% ratio and a 10ms interval at least one client must have
        // been flagged; flags stay set because nobody consumes them.
        let flagged = c.kicks.iter().filter(|f| f.load(Ordering::Relaxed)).count();
        assert!(flagged > 0, "no connection was kicked");
    }

    #[tokio::test]
    async fn churn_stops_immediately_when_stopped() {
        let c = ctx(4, 1.0);
        c.stop();
        tokio::time::timeout(Duration::from_secs(2), controller(c))
            .await
            .unwrap();
    }
}
