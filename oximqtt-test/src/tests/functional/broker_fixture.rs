//! Broker fixture for tests that need a dedicated broker process
//! started with a specific configuration (e.g. config-robustness e2e tests).
//!
//! Each fixture allocates a free TCP port, writes the given config into an
//! isolated temporary working directory (so the broker's `./oximqtt.toml`
//! auto-discovery cannot pick up unrelated files from the harness CWD), and
//! tears everything down on `Drop`.

use std::net::TcpListener;
use std::path::{Path, PathBuf};

use crate::broker::BrokerProcess;

/// Workspace root derived from the crate location
/// (`<root>/oximqtt-test` at compile time).
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Reserve a free ephemeral port by binding and releasing it.
pub fn free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").expect("failed to bind ephemeral port");
    l.local_addr().expect("no local addr").port()
}

/// A running broker with a private temp dir; cleaned up on drop.
pub struct TestBroker {
    process: BrokerProcess,
    temp_dir: PathBuf,
}

impl TestBroker {
    /// Start a broker listening on a fresh port with the given TOML config.
    ///
    /// The `addr` placeholder `{port}` in the config content is replaced with
    /// the reserved free port. Returns an error (instead of panicking) if the
    /// broker does not become healthy within the startup timeout.
    pub fn start(config_toml: &str) -> Result<Self, anyhow::Error> {
        let port = free_port();
        let addr = format!("127.0.0.1:{port}");
        let config_toml = config_toml.replace("{port}", &port.to_string());

        let temp_dir = std::env::temp_dir().join(format!("oximqtt-test-broker-{port}"));
        std::fs::create_dir_all(&temp_dir)?;
        // Second dir without any toml: prevents config auto-discovery from
        // picking up the config file's directory scan results unexpectedly.
        let work_dir = temp_dir.join("cwd");
        std::fs::create_dir_all(&work_dir)?;

        let config_path = temp_dir.join("harness.toml");
        std::fs::write(&config_path, config_toml)?;

        let binary = BrokerProcess::find_binary(Some(&workspace_root()));
        let process = BrokerProcess::with_config(binary, addr, Some(config_path))
            .with_work_dir(work_dir);

        let mut process = process;
        match process.start() {
            Ok(()) => Ok(Self { process, temp_dir }),
            Err(e) => {
                cleanup_dir(&temp_dir);
                Err(e)
            }
        }
    }

    /// Broker address (`127.0.0.1:PORT`) clients should connect to.
    pub fn addr(&self) -> &str {
        self.process.addr()
    }
}

impl Drop for TestBroker {
    fn drop(&mut self) {
        let _ = self.process.kill();
        cleanup_dir(&self.temp_dir);
    }
}

fn cleanup_dir(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

/// Run a broker to completion (expected to fail fast) and return
/// `(exit_success, stdout+stderr combined)`.
pub fn run_broker_to_exit(config_toml: &str) -> (bool, String) {
    let port = free_port();
    let temp_dir = std::env::temp_dir().join(format!("oximqtt-test-exit-{port}"));
    let _ = std::fs::create_dir_all(&temp_dir);
    let config_path = temp_dir.join("harness.toml");
    let _ = std::fs::write(
        &config_path,
        config_toml.replace("{port}", &port.to_string()),
    );

    let binary = BrokerProcess::find_binary(Some(&workspace_root()));
    let output = std::process::Command::new(binary)
        .arg("--config")
        .arg(&config_path)
        .current_dir(&temp_dir)
        .output();

    let _ = std::fs::remove_dir_all(&temp_dir);

    match output {
        Ok(o) => (
            o.status.success(),
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)),
        ),
        Err(e) => (false, format!("failed to spawn broker: {e}")),
    }
}
