//! `mqtt-bench` — standalone MQTT benchmark / load generator for OXIMQTT.
//!
//! The CLI is a superset of `rmqtt-bench`: same connection / subscription /
//! publication semantics and pattern syntax, plus a duration bound, latency
//! percentiles, QoS 2 support and a JSON report. Only MQTT framing comes from
//! the `oximqtt` crate; transport, client and statistics live here so the tool
//! never shares code with the broker runtime.
//!
//! ```text
//! mqtt-bench v3 -c 10000 -S -t iot/{no}                    # 10K subscriptions
//! mqtt-bench v3 -c 100 -P -t iot/{no} -R 0 10000 -I 10     # 100 publishers
//! mqtt-bench v5 -c 1000 -S -P -q 2 -d 30 --json r.json     # 30s QoS 2 run
//! ```

mod churn;
mod client;
mod mqtt;
mod options;
mod packets;
mod report;
mod stats;
mod transport;
mod window;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use tokio::task::JoinSet;
use tokio::time;

use crate::client::{run, Ctx};
use crate::options::Cli;
use crate::report::{write_json, Roster};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let (proto, mut args) = cli.into_args();
    args.validate()?;
    init_logging();

    eprintln!(
        "mqtt-bench {} ({} clients)",
        env!("CARGO_PKG_VERSION"),
        args.conns
    );
    eprintln!("  {}", args.describe(proto));

    let stop = Arc::new(AtomicBool::new(false));
    let ctx = Arc::new(Ctx::new(args.clone(), proto, stop.clone())?);
    let stats = ctx.stats.clone();

    let mut helpers = JoinSet::new();
    helpers.spawn(report::reporter(ctx.clone()));
    if args.control {
        helpers.spawn(churn::controller(ctx.clone()));
    }
    if args.duration > 0 {
        helpers.spawn(churn::scheduled_stop(ctx.clone(), args.duration));
    }
    helpers.spawn(churn::signal_stop(stop.clone()));

    let mut roster = Roster::new();
    for no in 0..args.conns {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let c = ctx.clone();
        roster.spawn(async move { run(c, no).await });
        if args.conn_interval > 0 {
            time::sleep(Duration::from_millis(args.conn_interval)).await;
        }
    }
    eprintln!(
        "  spawned {} clients, {} broker endpoints",
        roster.spawned,
        ctx.peers.len()
    );

    roster.wait(bounded_run_time(&args)).await;
    stop.store(true, Ordering::Relaxed);
    roster.abort_remaining().await;
    let elapsed = ctx.epoch.elapsed();

    // The helper tasks (reporter, churn, Ctrl-C waiter) are not expected to
    // return on their own, tearing them down must not extend the measured run.
    helpers.abort_all();
    helpers.shutdown().await;

    let report = stats.report(elapsed);
    println!();
    println!("{}", crate::stats::format_summary(&report));
    if let Some(path) = &args.json {
        write_json(path, proto, &args, &report)?;
        eprintln!("  json report written to {path}");
    }
    Ok(())
}

/// Wall clock budget for the run: the duration bound plus a generous margin
/// for the reconnect grace, or "forever" when unbounded.
fn bounded_run_time(args: &options::Opts) -> Option<Duration> {
    if args.duration > 0 {
        Some(Duration::from_secs(
            args.duration + args.handshake_timeout + 60,
        ))
    } else {
        None
    }
}

fn init_logging() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("warn,mqtt_bench=info,oximqtt=warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}
