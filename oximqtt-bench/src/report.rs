//! Reporting: the periodic console line, the client task roster and the JSON
//! summary export.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use tokio::task::JoinSet;
use tokio::time;

use crate::client::Ctx;
use crate::options::{Opts, Protocol};
use crate::stats::{format_line, Counters, Report, Stats};

/// Tracks the running client tasks and waits for them to finish.
pub struct Roster {
    set: JoinSet<()>,
    pub spawned: usize,
}

impl Roster {
    /// Empty roster.
    pub fn new() -> Self {
        Roster {
            set: JoinSet::new(),
            spawned: 0,
        }
    }

    /// Start one client task.
    pub fn spawn(&mut self, fut: impl std::future::Future<Output = ()> + Send + 'static) {
        self.set.spawn(fut);
        self.spawned += 1;
    }

    /// Wait until every client returned, or until `budget` elapses.
    pub async fn wait(&mut self, budget: Option<Duration>) {
        let Some(budget) = budget else {
            while self.set.join_next().await.is_some() {}
            return;
        };
        let deadline = time::Instant::now() + budget;
        while !self.set.is_empty() {
            match time::timeout_at(deadline, self.set.join_next()).await {
                Ok(None) => break,
                Ok(Some(_)) => continue,
                Err(_) => {
                    eprintln!(
                        "  {:?} budget exhausted, {} clients still running",
                        budget,
                        self.set.len()
                    );
                    break;
                }
            }
        }
    }

    /// Abort whatever is left and reap the tasks.
    pub async fn abort_remaining(&mut self) {
        self.set.abort_all();
        while self.set.join_next().await.is_some() {}
    }
}

/// Prints one stats line every `--output-interval` seconds until the run ends.
pub async fn reporter(ctx: Arc<Ctx>) {
    let stats: Arc<Stats> = ctx.stats.clone();
    let interval = Duration::from_secs(ctx.args.output_interval);
    let mut last = Counters::default();
    let mut last_at = Instant::now();
    loop {
        time::sleep(interval).await;
        if ctx.stopped() {
            break;
        }
        let now = Instant::now();
        let totals = stats.snapshot();
        let delta = totals.since(&last);
        println!(
            "{}",
            format_line(
                ctx.epoch.elapsed(),
                &totals,
                &delta,
                now - last_at,
                &stats.latency_stats()
            )
        );
        // Peek, do not drain: the final summary must still report it.
        if let Some(err) = stats.peek_last_err() {
            println!("              last error: {err}");
        }
        last = totals;
        last_at = now;
    }
}

/// Write the machine readable report next to the resolved configuration.
pub fn write_json(
    path: impl AsRef<Path>,
    proto: Protocol,
    args: &Opts,
    report: &Report,
) -> Result<()> {
    let path = path.as_ref();
    let doc = serde_json::json!({
        "tool": concat!("mqtt-bench-", env!("CARGO_PKG_VERSION")),
        "protocol": proto.to_string(),
        "config": args,
        "report": report,
    });
    let body = serde_json::to_string_pretty(&doc).map_err(|e| anyhow!("serialize report: {e}"))?;
    std::fs::write(path, body).map_err(|e| anyhow!("cannot write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::stats::Stats;
    #[tokio::test]
    async fn roster_waits_for_all_tasks() {
        let mut r = Roster::new();
        let n = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        for i in 0..5 {
            let n = n.clone();
            r.spawn(async move {
                time::sleep(Duration::from_millis(i * 5)).await;
                n.fetch_add(1, Ordering::SeqCst);
            });
        }
        assert_eq!(r.spawned, 5);
        r.wait(None).await;
        assert_eq!(n.load(Ordering::SeqCst), 5);
    }

    #[tokio::test]
    async fn roster_budget_expires_and_aborts() {
        let mut r = Roster::new();
        r.spawn(async {
            loop {
                time::sleep(Duration::from_millis(10)).await
            }
        });
        r.wait(Some(Duration::from_millis(50))).await;
        r.abort_remaining().await;
    }

    #[tokio::test]
    async fn reporter_emits_lines_and_stops() {
        let args = Opts {
            output_interval: 1,
            ..Default::default()
        };
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let ctx = Arc::new(Ctx::new(args, Protocol::V3, stop.clone()).unwrap());
        ctx.stats.conn_established();
        ctx.stats.record_latency(42);
        ctx.stats.error("kaboom");
        let handle = tokio::spawn(reporter(ctx.clone()));
        time::sleep(Duration::from_millis(1200)).await;
        ctx.stop();
        time::timeout(Duration::from_secs(3), handle)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ctx.stats.latency_stats().samples, 1);
    }

    #[test]
    fn write_json_roundtrip() {
        let dir = std::env::temp_dir().join(format!("mqtt-bench-json-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("report.json");
        let args = Opts {
            conns: 3,
            ..Default::default()
        };
        let report = Stats::new().report(Duration::from_secs(1));
        write_json(&file, Protocol::V5, &args, &report).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("\"protocol\": \"5.0\""), "{text}");
        assert!(text.contains("\"conns\": 3"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_json_reports_missing_dir() {
        let args = Opts::default();
        let report = Stats::new().report(Duration::from_secs(0));
        let err = write_json(
            "/nonexistent-dir-xyz/report.json",
            Protocol::V3,
            &args,
            &report,
        )
        .unwrap_err();
        assert!(err.to_string().contains("cannot write"), "{err}");
    }
}
