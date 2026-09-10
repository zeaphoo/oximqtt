//! Logger initialization for the OXIMQTT broker.
//!
//! Configures tracing-subscriber with console and/or file logging output,
//! supporting configurable log levels, time formatting (UTC+8 default), and
//! non-blocking file writing to prevent I/O from blocking the event loop.

use std::path::Path;
use std::sync::OnceLock;

use anyhow::Context;

use time::{format_description::FormatItem, macros::format_description, OffsetDateTime, UtcOffset};
use tracing_appender::non_blocking::{self, WorkerGuard};
use tracing_subscriber::{fmt, fmt::time::FormatTime, layer::SubscriberExt, prelude::*, EnvFilter};

use oximqtt::conf::logging::Log;
use oximqtt::Result;

/// Prevent log loss on process exit.
static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

/// Example:
/// 2026-05-16 19:00:07.487
static TIME_FORMAT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]");

struct LocalTimer {
    offset: UtcOffset,
}

impl LocalTimer {
    #[inline]
    fn new() -> Self {
        // Use UTC+8 (China Standard Time) as default offset since current_local_offset was removed in time 0.3.34+
        let offset = UtcOffset::from_hms(8, 0, 0).unwrap_or(UtcOffset::UTC);
        Self { offset }
    }
}

impl FormatTime for LocalTimer {
    #[inline]
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        let now = OffsetDateTime::now_utc().to_offset(self.offset);

        match now.format(TIME_FORMAT) {
            Ok(s) => w.write_str(&s),
            Err(_) => w.write_str("0000-00-00 00:00:00.000"),
        }
    }
}

#[inline]
fn build_env_filter(level: &str) -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level))
}

fn build_non_blocking_writer(dir: &str, file: &str) -> Result<non_blocking::NonBlocking> {
    if !Path::new(dir).exists() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("failed to create log directory {dir:?}"))?;
    }

    let file_appender = tracing_appender::rolling::never(dir, file);

    let (writer, guard) = non_blocking::NonBlockingBuilder::default()
        .lossy(false)
        .buffered_lines_limit(1024 * 1024)
        .finish(file_appender);

    let _ = LOG_GUARD.set(guard);

    Ok(writer)
}

/// Shared layer configuration.
macro_rules! common_layer {
    ($layer:expr) => {
        $layer.with_target(true).with_line_number(true).with_thread_ids(false).with_ansi(false)
    };
}

/// Initializes tracing logger.
///
/// Supported config:
/// - log.to: off | console | file | both
/// - log.level: trace | debug | info | warn | error
/// - log.dir
/// - log.file
pub fn logger_init(cfg: &Log) -> Result<()> {
    if cfg.to.off() {
        return Ok(());
    }

    let env_filter = build_env_filter(cfg.level.as_str());

    // File output is opt-in: it requires an explicit, non-empty `log.dir`.
    // Without one we stay on the console, so a default (or partial) config
    // never fails on an unwritable path such as `/var/log/oximqtt`.
    let file_enabled = cfg.to.file() && !cfg.dir.trim().is_empty();

    let file_layer = if file_enabled {
        let writer = build_non_blocking_writer(&cfg.dir, &cfg.file)?;
        Some(
            common_layer!(fmt::layer().with_writer(writer).with_timer(LocalTimer::new()))
                .with_filter(env_filter.clone()),
        )
    } else {
        None
    };

    // Fall back to the console when file output was requested but unavailable.
    let console_enabled = cfg.to.console() || !file_enabled;
    let console_layer = if console_enabled {
        Some(
            common_layer!(fmt::layer()
                .with_writer(std::io::stderr)
                .with_timer(LocalTimer::new())
                .compact())
            .with_filter(env_filter),
        )
    } else {
        None
    };

    tracing_subscriber::registry().with(file_layer).with(console_layer).try_init()?;

    if cfg.to.file() && !file_enabled {
        tracing::warn!(
            "log.to = {:?} requires a non-empty log.dir; falling back to console output",
            cfg.to
        );
    }

    Ok(())
}
