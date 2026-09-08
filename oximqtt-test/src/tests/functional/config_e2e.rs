//! Configuration-robustness end-to-end tests.
//!
//! Regression coverage for the "panic on optional config" bugs:
//! - the documented optional `[auth_jwt]` section must not be required;
//! - a partial `[auth_jwt]` section must fill remaining fields with defaults;
//! - an unusable/explicitly-wrong config must exit with a clear error,
//!   never with a panic;
//! - unknown sections must be ignored so the simplest possible config runs.

use std::time::{Duration, Instant};

use bytes::Bytes;

use crate::framework::context::TestContext;
use crate::framework::testcase::{TestCase, TestResult};
use crate::mqtt::common::QoS;
use crate::tests::functional::broker_fixture::{run_broker_to_exit, TestBroker};

const SUITE: &str = "functional_config";

/// HS256 token signed with the built-in default secret "oximqttsecret", exp far future.
const TOKEN_DEFAULT_SECRET: &str =
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJleHAiOjQwMDAwMDAwMDAsInN1YiI6Im94aW1xdHQifQ.Lff9V-ULHTE6nIt4rfliqrg3Hk8xPIw2I2jHE2zd2eE";
/// HS256 token signed with "testsecret", exp far future.
const TOKEN_TEST_VALID: &str =
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJleHAiOjQwMDAwMDAwMDAsInN1YiI6ImNvbXBhdCJ9.aLxHyooZzzNvICC01OPHkbvbYdV0DtVtcj1hZQ1jdFM";
/// HS256 token signed with "testsecret", already expired.
const TOKEN_TEST_EXPIRED: &str =
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJleHAiOjEwMCwic3ViIjoiY29tcGF0In0.lgPPT31IGHdWBhcU4Zqxp2cjVFHdKsl-EAFxJp_fAng";
/// HS256 token signed with "testsecret", no exp claim.
const TOKEN_TEST_NOEXP: &str =
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJjb21wYXQifQ.dGuqSodn0WGTvrcgyGh3WOq5gBaUZ2pCA3Sr7j-oj6Y";

fn minimal_config(auth_lines: &str) -> String {
    format!("listener.tcp.external.addr = \"127.0.0.1:{{port}}\"\n{auth_lines}")
}

/// Try to connect a v3.1.1 client, optionally carrying a JWT in the password.
async fn connect_with_password(addr: &str, client_id: &str, token: Option<&str>) -> bool {
    let password = token.map(|t| Bytes::from(t.as_bytes().to_vec()));
    let client = crate::mqtt::v311::MqttV311Client::connect_with_options(
        addr,
        client_id,
        Duration::from_secs(5),
        true,
        60,
        None,
        None,
        password,
    )
    .await;
    match client {
        Ok(c) => {
            let _ = c.disconnect().await;
            true
        }
        Err(_) => false,
    }
}

/// Full QoS 1 round trip (subscribe + self publish) against a broker addr.
async fn pubsub_roundtrip(addr: &str) -> anyhow::Result<()> {
    let mut sub = crate::mqtt::v311::MqttV311Client::connect(addr, "config-e2e-sub", Duration::from_secs(5))
        .await?;
    sub.subscribe("config/e2e/roundtrip", QoS::AtLeastOnce).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    sub.publish("config/e2e/roundtrip", b"config-e2e", QoS::AtLeastOnce, false).await?;
    let msg = tokio::time::timeout(Duration::from_secs(5), sub.recv_message()).await??;
    let _ = sub.disconnect().await;
    anyhow::ensure!(msg.payload.as_ref() == b"config-e2e", "unexpected payload: {:?}", msg.payload);
    Ok(())
}

/// The absolute minimal config (single listener line, no `[auth_jwt]`,
/// no other sections) must start and serve plain anonymous MQTT traffic.
///
/// Regression: previously the broker panicked with
/// `builtins init failed: missing field hmac_base64` when auth_jwt was absent.
pub struct MinimalConfigNoAuthJwtTest;

impl TestCase for MinimalConfigNoAuthJwtTest {
    fn name(&self) -> &str {
        "config_minimal_no_auth_jwt"
    }

    fn execute(&self, _ctx: &mut TestContext) -> TestResult {
        let start = Instant::now();
        let result = run(self.name());
        verdict(self.name(), start, result)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }
}

fn run(_name: &str) -> anyhow::Result<()> {
    let broker = TestBroker::start(&minimal_config(""))?;
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        // anonymous connect allowed when JWT module is off
        anyhow::ensure!(
            connect_with_password(broker.addr(), "config-min-anon", None).await,
            "anonymous connect rejected while [auth_jwt] section is absent"
        );
        pubsub_roundtrip(broker.addr()).await?;
        // tokens are NOT validated while the module is disabled
        anyhow::ensure!(
            connect_with_password(broker.addr(), "config-min-garbage", Some("not-a-jwt")).await,
            "garbage password rejected although auth_jwt module should be disabled"
        );
        Ok(())
    })
}

/// `[auth_jwt]` present but empty: the module is enabled and every field
/// falls back to its documented default (secret "oximqttsecret", exp check on).
pub struct AuthJwtEmptySectionDefaultsTest;

impl TestCase for AuthJwtEmptySectionDefaultsTest {
    fn name(&self) -> &str {
        "config_auth_jwt_empty_section_defaults"
    }

    fn execute(&self, _ctx: &mut TestContext) -> TestResult {
        let start = Instant::now();
        let result = (|| -> anyhow::Result<()> {
            let broker = TestBroker::start(&minimal_config("[auth_jwt]\n"))?;
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                anyhow::ensure!(
                    connect_with_password(broker.addr(), "jwt-empty-default-token", Some(TOKEN_DEFAULT_SECRET))
                        .await,
                    "token signed with the documented default secret was rejected"
                );
                anyhow::ensure!(
                    !connect_with_password(broker.addr(), "jwt-empty-anon", None).await,
                    "anonymous connect accepted although auth_jwt is enabled"
                );
                anyhow::ensure!(
                    !connect_with_password(broker.addr(), "jwt-empty-wrongkey", Some(TOKEN_TEST_VALID)).await,
                    "token signed with a different key was accepted"
                );
                Ok(())
            })
        })();
        verdict(self.name(), start, result)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }
}

/// A partial `[auth_jwt]` section (only `hmac_secret`, no `hmac_base64`) must
/// parse with defaults and enforce JWT auth correctly.
///
/// Regression: missing `#[serde(default)]` on `hmac_base64` used to abort startup.
pub struct AuthJwtPartialSectionTest;

impl TestCase for AuthJwtPartialSectionTest {
    fn name(&self) -> &str {
        "config_auth_jwt_partial_section"
    }

    fn execute(&self, _ctx: &mut TestContext) -> TestResult {
        let start = Instant::now();
        let result = (|| -> anyhow::Result<()> {
            let broker = TestBroker::start(&minimal_config("auth_jwt.hmac_secret = \"testsecret\"\n"))?;
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                anyhow::ensure!(
                    connect_with_password(broker.addr(), "jwt-partial-valid", Some(TOKEN_TEST_VALID)).await,
                    "valid JWT rejected with partial auth_jwt config"
                );
                anyhow::ensure!(
                    connect_with_password(broker.addr(), "jwt-partial-noexp", Some(TOKEN_TEST_NOEXP)).await,
                    "token without exp rejected (exp not in required claims)"
                );
                anyhow::ensure!(
                    !connect_with_password(broker.addr(), "jwt-partial-expired", Some(TOKEN_TEST_EXPIRED)).await,
                    "expired JWT accepted (default validate_claims.exp should reject it)"
                );
                anyhow::ensure!(
                    !connect_with_password(broker.addr(), "jwt-partial-forged", Some("aaa.bbb.ccc")).await,
                    "forged JWT accepted"
                );
                Ok(())
            })
        })();
        verdict(self.name(), start, result)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }
}

/// `encrypt = "public-key"` without `public_key` is a fatal misconfiguration,
/// but the process must exit with a clear error message, never a panic.
pub struct AuthJwtMissingPubKeyCleanExitTest;

impl TestCase for AuthJwtMissingPubKeyCleanExitTest {
    fn name(&self) -> &str {
        "config_auth_jwt_missing_pubkey_clean_exit"
    }

    fn execute(&self, _ctx: &mut TestContext) -> TestResult {
        let start = Instant::now();
        let config = minimal_config("auth_jwt.encrypt = \"public-key\"\n");
        let (ok, output) = run_broker_to_exit(&config);
        let result = (|| -> anyhow::Result<()> {
            anyhow::ensure!(!ok, "broker unexpectedly started with missing public_key");
            anyhow::ensure!(
                output.contains("public_key"),
                "startup error does not mention public_key, output: {output}"
            );
            anyhow::ensure!(!output.contains("panicked"), "broker panicked, output: {output}");
            Ok(())
        })();
        verdict(self.name(), start, result)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }
}

/// A wrong value type in an option is a fatal misconfiguration,
/// reported as a parse error — not a panic.
pub struct AuthJwtWrongTypeCleanExitTest;

impl TestCase for AuthJwtWrongTypeCleanExitTest {
    fn name(&self) -> &str {
        "config_auth_jwt_wrong_type_clean_exit"
    }

    fn execute(&self, _ctx: &mut TestContext) -> TestResult {
        let start = Instant::now();
        let config = minimal_config("auth_jwt.from = 123\n");
        let (ok, output) = run_broker_to_exit(&config);
        let result = (|| -> anyhow::Result<()> {
            anyhow::ensure!(!ok, "broker unexpectedly started with invalid auth_jwt.from type");
            anyhow::ensure!(!output.contains("panicked"), "broker panicked, output: {output}");
            Ok(())
        })();
        verdict(self.name(), start, result)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }
}

/// Unknown sections/keys are ignored so a config may carry forward-compatible
/// or plugin-specific entries without breaking startup.
pub struct UnknownConfigSectionsIgnoredTest;

impl TestCase for UnknownConfigSectionsIgnoredTest {
    fn name(&self) -> &str {
        "config_unknown_sections_ignored"
    }

    fn execute(&self, _ctx: &mut TestContext) -> TestResult {
        let start = Instant::now();
        let result = (|| -> anyhow::Result<()> {
            let config =
                minimal_config("[some_future_section]\nanything = true\nmqtt.unknown_option = 42\n");
            let broker = TestBroker::start(&config)?;
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                anyhow::ensure!(
                    connect_with_password(broker.addr(), "unknown-sec-anon", None).await,
                    "broker with unknown config sections refused anonymous connections"
                );
                Ok(())
            })
        })();
        verdict(self.name(), start, result)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }
}

/// The retained-message store is bounded **by default** and the cap must
/// behave correctly when reached: existing topics keep refreshing, only
/// brand-new retained topics are dropped.
pub struct RetainerCapKeepsExistingTopicsFreshTest;

impl TestCase for RetainerCapKeepsExistingTopicsFreshTest {
    fn name(&self) -> &str {
        "config_retainer_cap_keeps_existing_topics_fresh"
    }

    fn execute(&self, _ctx: &mut TestContext) -> TestResult {
        let start = Instant::now();
        let result = (|| -> anyhow::Result<()> {
            let broker = TestBroker::start(&minimal_config("retainer.max_retained_messages = 2\n"))?;
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let pub1 = crate::mqtt::v311::MqttV311Client::connect(
                    broker.addr(),
                    "retcap-pub",
                    Duration::from_secs(5),
                )
                .await?;
                let base = "config/e2e/retcap";
                pub1.publish(&format!("{base}/a"), b"a1", QoS::AtLeastOnce, true).await?;
                pub1.publish(&format!("{base}/b"), b"b1", QoS::AtLeastOnce, true).await?;
                // cap = 2 reached; this NEW topic must be dropped…
                pub1.publish(&format!("{base}/c"), b"c1", QoS::AtLeastOnce, true).await?;
                // …while a refresh of an EXISTING topic must still be stored
                pub1.publish(&format!("{base}/a"), b"a2", QoS::AtLeastOnce, true).await?;
                tokio::time::sleep(Duration::from_millis(200)).await;

                let mut sub = crate::mqtt::v311::MqttV311Client::connect(
                    broker.addr(),
                    "retcap-sub",
                    Duration::from_secs(5),
                )
                .await?;
                sub.subscribe(&format!("{base}/#"), QoS::AtLeastOnce).await?;

                let mut payloads = Vec::new();
                while let Some(m) =
                    sub.recv_message_timeout(Duration::from_millis(1500)).await
                {
                    payloads.push(String::from_utf8_lossy(&m.payload).to_string());
                }
                let _ = sub.disconnect().await;
                let _ = pub1.disconnect().await;

                anyhow::ensure!(
                    payloads.iter().any(|p| p == "a2"),
                    "existing retained topic refresh lost, got {payloads:?}"
                );
                anyhow::ensure!(
                    payloads.iter().any(|p| p == "b1"),
                    "second retained topic missing, got {payloads:?}"
                );
                anyhow::ensure!(
                    !payloads.iter().any(|p| p == "c1") && !payloads.iter().any(|p| p == "a1"),
                    "cap violated or stale retained value resurfaced: {payloads:?}",
                );
                anyhow::ensure!(payloads.len() == 2, "expected exactly 2 retained messages, got {payloads:?}");
                Ok(())
            })
        })();
        verdict(self.name(), start, result)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }
}

fn verdict(name: &str, start: Instant, result: anyhow::Result<()>) -> TestResult {
    match result {
        Ok(()) => TestResult::passed(name, SUITE, start.elapsed()),
        Err(e) => TestResult::failed(name, SUITE, start.elapsed(), e.to_string()),
    }
}
