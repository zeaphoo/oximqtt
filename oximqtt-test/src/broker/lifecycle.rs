//! Broker process lifecycle management
//!
//! Starts oximqttd as a child process, manages its lifecycle,
//! and provides health checking. All operations are synchronous
//! to avoid nested runtime issues when called from within tests.

use std::path::PathBuf;
use std::process::Child;
use std::time::Duration;

use tracing::{info, warn};

use super::healthcheck::health_check_sync;

/// Default broker TCP address
const DEFAULT_BROKER_ADDR: &str = "127.0.0.1:1883";

/// Broker process manager (synchronous)
pub struct BrokerProcess {
    /// Path to the broker binary
    binary_path: PathBuf,
    /// Broker listen address for health check
    addr: String,
    /// Config file path (optional)
    config_path: Option<PathBuf>,
    /// Working directory for the broker process (optional).
    /// The broker also auto-discovers `./oximqtt.toml` relative to CWD, so an
    /// isolated directory prevents accidental config inheritance.
    work_dir: Option<PathBuf>,
    /// The running child process
    child: Option<Child>,
}

impl BrokerProcess {
    /// Create a new broker process manager
    ///
    /// Searches for the broker binary in target/release and target/debug
    pub fn new(workspace_root: Option<PathBuf>) -> Self {
        let binary_path = Self::find_binary(workspace_root.as_deref());
        Self {
            binary_path,
            addr: DEFAULT_BROKER_ADDR.to_string(),
            config_path: None,
            work_dir: None,
            child: None,
        }
    }

    /// Create with a specific binary path and address
    pub fn with_config(binary_path: PathBuf, addr: String, config_path: Option<PathBuf>) -> Self {
        Self { binary_path, addr, config_path, work_dir: None, child: None }
    }

    /// Set the working directory used when spawning the broker
    pub fn with_work_dir(mut self, work_dir: PathBuf) -> Self {
        self.work_dir = Some(work_dir);
        self
    }

    /// Find the broker binary
    pub fn find_binary(workspace_root: Option<&std::path::Path>) -> PathBuf {
        if let Some(root) = workspace_root {
            for dir in &["target/release", "target/debug"] {
                let path = root.join(dir).join("oximqttd.exe");
                if path.exists() {
                    info!("Found broker binary: {:?}", path);
                    return path;
                }
                // Also try without .exe (Linux/macOS)
                let path = root.join(dir).join("oximqttd");
                if path.exists() {
                    info!("Found broker binary: {:?}", path);
                    return path;
                }
            }
        }

        // Fallback: try current directory
        PathBuf::from("oximqttd")
    }

    /// Start the broker process (synchronous)
    pub fn start(&mut self) -> Result<(), anyhow::Error> {
        if self.child.is_some() {
            warn!("Broker already running, skipping start");
            return Ok(());
        }

        // Resolve relative binary paths against the harness CWD *before*
        // switching the child into an isolated working directory.
        let binary_path = if self.binary_path.is_relative() {
            std::env::current_dir()
                .map(|cwd| cwd.join(&self.binary_path))
                .unwrap_or_else(|_| self.binary_path.clone())
        } else {
            self.binary_path.clone()
        };
        if !binary_path.exists() {
            return Err(anyhow::anyhow!("broker binary not found at {:?}", binary_path));
        }

        info!("Starting broker: {:?}", binary_path);

        let mut cmd = std::process::Command::new(&binary_path);
        cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());

        if let Some(ref work_dir) = self.work_dir {
            cmd.current_dir(work_dir);
        }

        if let Some(ref config) = self.config_path {
            // oximqttd CLI flag is `--config` (short `-f`)
            cmd.arg("--config").arg(config);
        }

        let child = cmd.spawn()?;
        self.child = Some(child);

        // Wait for broker to become healthy
        let healthy = self.wait_healthy(Duration::from_secs(10));
        if healthy {
            info!("Broker is healthy at {}", self.addr);
            Ok(())
        } else {
            let _ = self.kill();
            Err(anyhow::anyhow!("broker failed to become healthy within timeout"))
        }
    }

    /// Wait for the broker to become healthy (synchronous polling)
    pub fn wait_healthy(&self, timeout: Duration) -> bool {
        let start = std::time::Instant::now();
        let check_interval = Duration::from_millis(200);

        while start.elapsed() < timeout {
            if health_check_sync(&self.addr, Duration::from_secs(2)) {
                return true;
            }
            std::thread::sleep(check_interval);
        }
        false
    }

    /// Check if the broker is healthy (synchronous)
    pub fn health_check(&self) -> bool {
        health_check_sync(&self.addr, Duration::from_secs(2))
    }

    /// Stop the broker gracefully
    pub fn stop(&mut self) -> Result<(), anyhow::Error> {
        if let Some(mut child) = self.child.take() {
            info!("Stopping broker (PID: {:?})", child.id());
            let _ = child.kill();
            let _ = child.wait();
            info!("Broker stopped");
        }
        Ok(())
    }

    /// Restart the broker
    pub fn restart(&mut self) -> Result<(), anyhow::Error> {
        self.stop()?;
        // Small delay to let ports free up
        std::thread::sleep(Duration::from_millis(500));
        self.start()
    }

    /// Kill the broker immediately
    pub fn kill(&mut self) -> Result<(), anyhow::Error> {
        if let Some(mut child) = self.child.take() {
            info!("Killing broker (PID: {:?})", child.id());
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }

    /// Get the broker address
    pub fn addr(&self) -> &str {
        &self.addr
    }

    /// Get the broker binary path
    pub fn binary_path(&self) -> &std::path::Path {
        &self.binary_path
    }

    /// Check if the broker process is running
    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }
}

impl Drop for BrokerProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // Best effort cleanup
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
