//! Test support for the end-to-end benchmark tests: starts a real `oximqttd`
//! on a free port and runs the `mqtt-bench` binary against it.

#![allow(dead_code, reason = "shared helpers, not every test uses all of them")]

use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Limits how many broker-backed tests run at the same time. Each broker plus
/// benchmark process uses several cores' worth of threads; without a gate the
/// whole suite oversubscribes the machine and slow clients start losing
/// messages, which turns load into false failures.
static GATE: Gate = Gate::new(2);

/// Counting semaphore with a condvar, so the tests stay dependency free.
struct Gate {
    inner: std::sync::Mutex<usize>,
    ready: std::sync::Condvar,
    total: usize,
}

/// Held for the duration of one test.
pub struct Permit<'a>(&'a Gate);

impl Gate {
    const fn new(total: usize) -> Self {
        Gate {
            inner: std::sync::Mutex::new(total),
            ready: std::sync::Condvar::new(),
            total,
        }
    }

    fn acquire(&self) -> Permit<'_> {
        let mut free = self.inner.lock().expect("gate poisoned");
        while *free == 0 {
            free = self.ready.wait(free).expect("gate poisoned");
        }
        *free -= 1;
        Permit(self)
    }

    fn release(&self) {
        *self.inner.lock().expect("gate poisoned") += 1;
        self.ready.notify_one();
    }
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        self.0.release();
    }
}

/// A broker started for one test; killed when dropped.
pub struct Broker {
    child: Child,
    addr: String,
    temp_dir: PathBuf,
    _permit: Permit<'static>,
}

impl Broker {
    /// Start `oximqttd` on a freshly allocated port with `extra` appended to
    /// the minimal configuration.
    pub fn with_config(extra: &str) -> Broker {
        // Blocking on the gate is intended: it serialises the heavy tests.
        let binary = binary("oximqttd");
        let port = free_port();
        let addr = format!("127.0.0.1:{port}");

        let temp_dir = std::env::temp_dir().join(format!("mqtt-bench-test-{port}"));
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");
        // An empty working directory keeps the broker from auto-discovering the
        // repository `oximqtt.toml`.
        let work_dir = temp_dir.join("cwd");
        std::fs::create_dir_all(&work_dir).expect("create work dir");

        let config = temp_dir.join("broker.toml");
        let content = format!(
            "listener.tcp.external.addr = \"{addr}\"\nlog.to = \"console\"\nlog.level = \"warn\"\n\
             listener.tcp.external.nodelay = true\n{extra}"
        );
        std::fs::write(&config, content).expect("write broker config");

        let child = Command::new(&binary)
            .arg("--config")
            .arg(&config)
            .current_dir(&work_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("cannot start {}: {e}", binary.display()));

        let broker = Broker {
            child,
            addr,
            temp_dir,
            _permit: GATE.acquire(),
        };
        broker.wait_until_listening();
        broker
    }

    fn wait_until_listening(&self) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if TcpStream::connect(&self.addr).is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("broker did not start listening on {}", self.addr);
    }

    pub fn addr(&self) -> &str {
        &self.addr
    }
}

impl Drop for Broker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.temp_dir);
    }
}

/// Locate a built binary, preferring the profile the tests were built in.
pub fn binary(name: &str) -> PathBuf {
    if let Ok(explicit) = std::env::var(format!("MQTT_BENCH_{}", name.to_uppercase())) {
        return PathBuf::from(explicit);
    }
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let root = workspace_root();
    for candidate in [
        root.join("target").join(profile).join(name),
        root.join("target").join("release").join(name),
        root.join("target").join("debug").join(name),
    ] {
        if candidate.exists() {
            return candidate;
        }
    }
    panic!(
        "{} not found - run `cargo build --release -p oximqtt-bench -p oximqttd` first \
         (or set MQTT_BENCH_{})",
        name,
        name.to_uppercase()
    );
}

/// Workspace root: `CARGO_MANIFEST_DIR` is `<root>/oximqtt-bench`.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// A free TCP port, allocated by binding 0 and releasing it immediately.
pub fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port()
}

/// One benchmark invocation and its parsed report.
pub struct Bench {
    args: Vec<String>,
    binary: PathBuf,
    json_path: PathBuf,
}

/// A parsed `mqtt-bench --json` report.
#[derive(Debug, serde::Deserialize)]
pub struct Report {
    pub protocol: String,
    pub report: ReportBody,
}

#[derive(Debug, serde::Deserialize)]
pub struct ReportBody {
    pub elapsed_secs: f64,
    pub counters: Counters,
    pub avg_rate: Rates,
    pub latency: Latency,
    pub last_err: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct Counters {
    pub conn_attempts: u64,
    pub conn_ok: u64,
    pub conn_fail: u64,
    pub active_conns: u64,
    pub subs: u64,
    pub sends: u64,
    pub recvs: u64,
    pub pub_acks: u64,
    pub ack_timeouts: u64,
    pub reconnects: u64,
    pub errors: u64,
    pub dups: u64,
    pub recvs_retained: u64,
}

#[derive(Debug, serde::Deserialize)]
pub struct Rates {
    pub conn: f64,
    pub send: f64,
    pub recv: f64,
}

#[derive(Debug, serde::Deserialize)]
pub struct Latency {
    pub samples: u64,
    pub mean_us: u64,
    pub p50_us: u64,
    pub p99_us: u64,
    pub max_us: u64,
}

impl Bench {
    /// Prepare a run against `broker`. Each instance gets its own report file,
    /// tests run in parallel within one process.
    pub fn new(broker_addr: &str) -> Bench {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let id = std::process::id();
        Bench {
            args: vec![
                "--addrs".to_owned(),
                broker_addr.to_owned(),
                "-o".to_owned(),
                "1".to_owned(),
            ],
            binary: binary("mqtt-bench"),
            json_path: reports_dir().join(format!("report-{id}-{seq}.json")),
        }
    }

    /// Append arguments.
    pub fn arg(mut self, a: impl Into<String>, v: impl Into<String>) -> Bench {
        self.args.push(a.into());
        self.args.push(v.into());
        self
    }

    /// Append a switch.
    pub fn flag(mut self, f: impl Into<String>) -> Bench {
        self.args.push(f.into());
        self
    }

    /// Subcommand + flags as one string (handy for `v3 -c 10 -S`).
    pub fn line(mut self, cmdline: &str) -> Bench {
        self.args
            .extend(cmdline.split_whitespace().map(str::to_owned));
        self
    }

    /// Run to completion and return the parsed report.
    pub fn run(&self, subcommand: &str) -> (Output, Report) {
        let _ = std::fs::remove_file(&self.json_path);
        let output = Command::new(&self.binary)
            .arg(subcommand)
            .args(&self.args)
            .arg("--json")
            .arg(&self.json_path)
            .output()
            .unwrap_or_else(|e| panic!("cannot start {}: {e}", self.binary.display()));
        let report = self.read_report();
        (output, report)
    }

    /// Run expecting failure (invalid options, unreachable broker, ...).
    pub fn run_failing(&self, subcommand: &str) -> Output {
        let _ = std::fs::remove_file(&self.json_path);
        Command::new(&self.binary)
            .arg(subcommand)
            .args(&self.args)
            .arg("--json")
            .arg(&self.json_path)
            .output()
            .expect("run mqtt-bench")
    }

    /// Raw argument vector of the prepared run.
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Report file used by [`Bench::run`].
    pub fn json_path(&self) -> &Path {
        &self.json_path
    }

    /// Spawn without waiting, for overlapping runs (fan-out, churn, ...).
    pub fn spawn(&self, subcommand: &str) -> Child {
        let _ = std::fs::remove_file(&self.json_path);
        Command::new(&self.binary)
            .arg(subcommand)
            .args(&self.args)
            .arg("--json")
            .arg(&self.json_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("cannot start {}: {e}", self.binary.display()))
    }

    /// Spawn a run writing its report to a distinct path so several runs can
    /// be in flight at once.
    pub fn spawn_to(&self, subcommand: &str, json_path: &Path) -> Child {
        let _ = std::fs::remove_file(json_path);
        Command::new(&self.binary)
            .arg(subcommand)
            .args(&self.args)
            .arg("--json")
            .arg(json_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("cannot start {}: {e}", self.binary.display()))
    }

    /// Wait for a spawned run and parse its report.
    pub fn finish(child: &mut Child, json_path: &Path) -> Report {
        let status = child.wait().expect("wait for bench");
        assert!(status.success(), "mqtt-bench exited with {status}");
        parse_report(json_path)
    }

    fn read_report(&self) -> Report {
        parse_report(&self.json_path)
    }
}

/// Read and parse a report file written by `--json`.
pub fn parse_report(path: &Path) -> Report {
    let body = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read report {}: {e}", path.display()));
    serde_json::from_str(&body).unwrap_or_else(|e| panic!("bad report {body}: {e}"))
}

/// A `127.0.0.1:port` address nothing is listening on.
pub fn free_addr() -> String {
    format!("127.0.0.1:{}", free_port())
}

/// Directory holding the JSON reports of this test process.
fn reports_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mqtt-bench-reports-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create reports dir");
    dir
}

/// Report path for a run that is spawned explicitly, e.g. fan-out tests.
pub fn report_path(tag: &str) -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    reports_dir().join(format!("report-{tag}-{seq}.json"))
}
