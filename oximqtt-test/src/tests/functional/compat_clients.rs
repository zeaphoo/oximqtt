//! Multi-client compatibility tests.
//!
//! Exercise the broker with MQTT 3.1, 3.1.1 and 5.0 clients used
//! concurrently against the same broker instance:
//! - concurrent connections of all three protocol versions,
//! - cross-version publish/subscribe (v3 <-> v3.1.1 <-> v5),
//! - retained messages published by one version and consumed by others,
//! - wildcard topic matching across versions,
//! - session takeover when a client of a different version reuses the
//!   same client-id,
//! - a mixed crowd of many clients exchanging a fan-out burst.
//!
//! These run against the broker managed by the harness (`ctx.config.broker_addr`,
//! anonymous access, no auth module).

use std::time::{Duration, Instant};

use oximqtt::codec::v3::QoS;

use crate::framework::context::TestContext;
use crate::framework::testcase::{TestCase, TestResult};
use crate::mqtt::v3::MqttV3Client;
use crate::mqtt::v311::MqttV311Client;
use crate::mqtt::v5::MqttV5Client;

const SUITE: &str = "compat";
const CONN_TIMEOUT: Duration = Duration::from_secs(5);
const RECV_TIMEOUT: Duration = Duration::from_secs(5);

fn verdict(name: &str, start: Instant, result: anyhow::Result<()>) -> TestResult {
    match result {
        Ok(()) => TestResult::passed(name, SUITE, start.elapsed()),
        Err(e) => TestResult::failed(name, SUITE, start.elapsed(), e.to_string()),
    }
}

fn unique_tag() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..8].to_string()
}

fn unique_id(prefix: &str) -> String {
    format!("{prefix}-{}", unique_tag())
}

/// All three protocol versions coexist: concurrent connect/ping/disconnect.
pub struct CompatConcurrentMultiVersionTest;

impl TestCase for CompatConcurrentMultiVersionTest {
    fn name(&self) -> &str {
        "compat_concurrent_multi_version"
    }

    fn execute(&self, ctx: &mut TestContext) -> TestResult {
        let start = Instant::now();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result: anyhow::Result<()> = rt.block_on(async {
            let addr = ctx.config.broker_addr.clone();
            let id3a = unique_id("compat-v3");
            let id3b = unique_id("compat-v3");
            let id311a = unique_id("compat-v311");
            let id311b = unique_id("compat-v311");
            let id5a = unique_id("compat-v5");
            let id5b = unique_id("compat-v5");
            let (v3a, v3b, v311a, v311b, v5a, v5b) = tokio::join!(
                MqttV3Client::connect(&addr, &id3a, CONN_TIMEOUT),
                MqttV3Client::connect(&addr, &id3b, CONN_TIMEOUT),
                MqttV311Client::connect(&addr, &id311a, CONN_TIMEOUT),
                MqttV311Client::connect(&addr, &id311b, CONN_TIMEOUT),
                MqttV5Client::connect(&addr, &id5a, CONN_TIMEOUT),
                MqttV5Client::connect(&addr, &id5b, CONN_TIMEOUT),
            );
            let (v3a, v3b) = (v3a?, v3b?);
            let (v311a, v311b) = (v311a?, v311b?);
            let (v5a, v5b) = (v5a?, v5b?);

            anyhow::ensure!(v3a.is_connected() && v3b.is_connected(), "v3 client not connected");
            anyhow::ensure!(
                v311a.is_connected() && v311b.is_connected(),
                "v3.1.1 client not connected"
            );
            anyhow::ensure!(v5a.is_connected() && v5b.is_connected(), "v5 client not connected");

            v311a.ping().await?;
            v311b.ping().await?;
            v5a.ping().await?;
            v5b.ping().await?;

            v3a.disconnect().await?;
            v3b.disconnect().await?;
            v311a.disconnect().await?;
            v311b.disconnect().await?;
            v5a.disconnect().await?;
            v5b.disconnect().await?;
            Ok(())
        });
        verdict(self.name(), start, result)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }
}

/// Every version can publish for every other version:
/// 4 inbound messages (one per version/QoS mix), each subscriber must get all.
pub struct CompatCrossVersionPubSubTest;

impl TestCase for CompatCrossVersionPubSubTest {
    fn name(&self) -> &str {
        "compat_cross_version_pubsub"
    }

    fn execute(&self, ctx: &mut TestContext) -> TestResult {
        let start = Instant::now();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result: anyhow::Result<()> = rt.block_on(async {
            let addr = ctx.config.broker_addr.clone();
            let topic = format!("compat/xver/{}", unique_tag());

            let mut sub3 = MqttV3Client::connect(&addr, &unique_id("xver-sub3"), CONN_TIMEOUT).await?;
            let mut sub311 = MqttV311Client::connect(&addr, &unique_id("xver-sub311"), CONN_TIMEOUT).await?;
            let mut sub5 = MqttV5Client::connect(&addr, &unique_id("xver-sub5"), CONN_TIMEOUT).await?;

            sub3.subscribe(&topic).await?;
            sub311.subscribe(&topic, QoS::AtLeastOnce).await?;
            sub5.subscribe(&topic, QoS::AtLeastOnce).await?;
            tokio::time::sleep(Duration::from_millis(150)).await;

            let pub311 = MqttV311Client::connect(&addr, &unique_id("xver-pub311"), CONN_TIMEOUT).await?;
            let pub5 = MqttV5Client::connect(&addr, &unique_id("xver-pub5"), CONN_TIMEOUT).await?;
            let pub3 = MqttV3Client::connect(&addr, &unique_id("xver-pub3"), CONN_TIMEOUT).await?;

            pub311.publish(&topic, b"from-v311-q0", QoS::AtMostOnce, false).await?;
            pub311.publish(&topic, b"from-v311-q1", QoS::AtLeastOnce, false).await?;
            pub5.publish(&topic, b"from-v5-q2", QoS::ExactlyOnce, false).await?;
            pub3.publish(&topic, b"from-v3-q0").await?;

            let mut payloads3 = Vec::new();
            let mut payloads311 = Vec::new();
            let mut payloads5 = Vec::new();
            for _ in 0..5 {
                if let Some(m) = sub3.recv_message_timeout(Duration::from_millis(1500)).await {
                    payloads3.push(m.payload.to_vec());
                }
                if let Some(m) = sub311.recv_message_timeout(Duration::from_millis(1500)).await {
                    payloads311.push(m.payload.to_vec());
                }
                if let Some(m) = sub5.recv_message_timeout(Duration::from_millis(1500)).await {
                    payloads5.push(m.payload.to_vec());
                }
            }

            for (who, got) in [("v3", &payloads3), ("v311", &payloads311), ("v5", &payloads5)] {
                for expected in
                    [b"from-v311-q0".as_slice(), b"from-v311-q1".as_slice(), b"from-v5-q2".as_slice(), b"from-v3-q0".as_slice()]
                {
                    anyhow::ensure!(
                        got.iter().any(|p| p.as_slice() == expected),
                        "{who} subscriber missed {:?}",
                        String::from_utf8_lossy(expected)
                    );
                }
            }

            pub3.disconnect().await?;
            pub311.disconnect().await?;
            pub5.disconnect().await?;
            sub3.disconnect().await?;
            sub311.disconnect().await?;
            sub5.disconnect().await?;
            Ok(())
        });
        verdict(self.name(), start, result)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(40)
    }
}

/// Retained messages published by one version must be delivered to
/// subscribers of every other version.
pub struct CompatRetainedCrossVersionTest;

impl TestCase for CompatRetainedCrossVersionTest {
    fn name(&self) -> &str {
        "compat_retained_cross_version"
    }

    fn execute(&self, ctx: &mut TestContext) -> TestResult {
        let start = Instant::now();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result: anyhow::Result<()> = rt.block_on(async {
            let addr = ctx.config.broker_addr.clone();
            let topic = format!("compat/ret/{}", unique_tag());

            let pub5 = MqttV5Client::connect(&addr, &unique_id("ret-pub5"), CONN_TIMEOUT).await?;
            pub5.publish(&topic, b"retained-by-v5", QoS::AtLeastOnce, true).await?;
            tokio::time::sleep(Duration::from_millis(200)).await;

            // fresh subscribers of the other two versions must get the retained copy
            let mut sub311 = MqttV311Client::connect(&addr, &unique_id("ret-sub311"), CONN_TIMEOUT).await?;
            sub311.subscribe(&topic, QoS::AtLeastOnce).await?;
            let mut got311 = false;
            if let Some(m) = sub311.recv_message_timeout(RECV_TIMEOUT).await {
                got311 = m.payload.as_ref() == b"retained-by-v5";
            }
            sub311.disconnect().await?;
            anyhow::ensure!(got311, "v3.1.1 subscriber missed the v5 retained message");

            let mut sub3 = MqttV3Client::connect(&addr, &unique_id("ret-sub3"), CONN_TIMEOUT).await?;
            sub3.subscribe(&topic).await?;
            let mut got3 = false;
            if let Some(m) = sub3.recv_message_timeout(RECV_TIMEOUT).await {
                got3 = m.payload.as_ref() == b"retained-by-v5";
            }
            sub3.disconnect().await?;
            anyhow::ensure!(got3, "v3.1 subscriber missed the v5 retained message");

            // cleanup: empty retained payload removes the stored message
            pub5.publish(&topic, b"", QoS::AtLeastOnce, true).await?;
            pub5.disconnect().await?;
            Ok(())
        });
        verdict(self.name(), start, result)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(40)
    }
}

/// Wildcard subscriptions (`+` / `#`) must match identically regardless of
/// the version of publisher or subscriber.
pub struct CompatWildcardCrossVersionTest;

impl TestCase for CompatWildcardCrossVersionTest {
    fn name(&self) -> &str {
        "compat_wildcard_cross_version"
    }

    fn execute(&self, ctx: &mut TestContext) -> TestResult {
        let start = Instant::now();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result: anyhow::Result<()> = rt.block_on(async {
            let addr = ctx.config.broker_addr.clone();
            let root = format!("compat/wl/{}", unique_tag());

            let mut sub_hash311 =
                MqttV311Client::connect(&addr, &unique_id("wl-sub311"), CONN_TIMEOUT).await?;
            let mut sub_plus5 = MqttV5Client::connect(&addr, &unique_id("wl-sub5"), CONN_TIMEOUT).await?;
            let mut sub_all3 = MqttV3Client::connect(&addr, &unique_id("wl-sub3"), CONN_TIMEOUT).await?;

            sub_hash311.subscribe(&format!("{root}/w/#"), QoS::AtLeastOnce).await?;
            sub_plus5.subscribe(&format!("{root}/m/+/x"), QoS::AtLeastOnce).await?;
            sub_all3.subscribe(&format!("{root}/#")).await?;
            tokio::time::sleep(Duration::from_millis(150)).await;

            let pub3 = MqttV3Client::connect(&addr, &unique_id("wl-pub3"), CONN_TIMEOUT).await?;
            let pub5 = MqttV5Client::connect(&addr, &unique_id("wl-pub5"), CONN_TIMEOUT).await?;

            // matches `w/#`
            pub3.publish(&format!("{root}/w/a/b"), b"hash-only").await?;
            // matches `m/+/x`
            pub5.publish(&format!("{root}/m/1/x"), b"plus-only", QoS::AtLeastOnce, false).await?;
            // matches only the root `#` filter
            pub5.publish(&format!("{root}/other"), b"root-only", QoS::AtLeastOnce, false).await?;

            let mut got311 = Vec::new();
            let mut got5 = Vec::new();
            let mut got3 = Vec::new();
            for _ in 0..3 {
                if let Some(m) = sub_hash311.recv_message_timeout(Duration::from_millis(1500)).await {
                    got311.push(m.payload.to_vec());
                }
                if let Some(m) = sub_plus5.recv_message_timeout(Duration::from_millis(1500)).await {
                    got5.push(m.payload.to_vec());
                }
                if let Some(m) = sub_all3.recv_message_timeout(Duration::from_millis(1500)).await {
                    got3.push(m.payload.to_vec());
                }
            }

            anyhow::ensure!(
                got311.iter().any(|p| p == b"hash-only") && !got311.iter().any(|p| p == b"plus-only"),
                "v3.1.1 `#` filter mismatch: {got311:?}"
            );
            anyhow::ensure!(
                got5.iter().any(|p| p == b"plus-only") && !got5.iter().any(|p| p == b"hash-only"),
                "v5 `+` filter mismatch: {got5:?}"
            );
            anyhow::ensure!(
                got3.iter().any(|p| p == b"hash-only")
                    && got3.iter().any(|p| p == b"plus-only")
                    && got3.iter().any(|p| p == b"root-only"),
                "v3.1 `#` root filter mismatch: {got3:?}"
            );

            pub3.disconnect().await?;
            pub5.disconnect().await?;
            sub_hash311.disconnect().await?;
            sub_plus5.disconnect().await?;
            sub_all3.disconnect().await?;
            Ok(())
        });
        verdict(self.name(), start, result)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(40)
    }
}

/// Session takeover: a v5 client stealing the session of a v3.1.1 client
/// (same client-id, different protocol version) must kick the old one off
/// and the new session must be fully functional.
pub struct CompatTakeoverAcrossVersionsTest;

impl TestCase for CompatTakeoverAcrossVersionsTest {
    fn name(&self) -> &str {
        "compat_takeover_across_versions"
    }

    fn execute(&self, ctx: &mut TestContext) -> TestResult {
        let start = Instant::now();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result: anyhow::Result<()> = rt.block_on(async {
            let addr = ctx.config.broker_addr.clone();
            let shared_id = unique_id("compat-takeover");

            let old = MqttV311Client::connect_with_options(
                &addr,
                &shared_id,
                CONN_TIMEOUT,
                false, // clean_session = false
                60,
                None,
                None,
                None,
            )
            .await?;
            anyhow::ensure!(old.is_connected(), "original v3.1.1 session not connected");

            let mut fresh = MqttV5Client::connect(&addr, &shared_id, CONN_TIMEOUT).await?;

            // the previous connection must be gone
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            while old.is_connected() && tokio::time::Instant::now() < deadline {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            anyhow::ensure!(
                !old.is_connected(),
                "v5 takeover did not disconnect the v3.1.1 client"
            );

            // the new session is fully functional
            let topic = format!("compat/takeover/{}", unique_tag());
            fresh.subscribe(&topic, QoS::AtLeastOnce).await?;
            tokio::time::sleep(Duration::from_millis(100)).await;
            fresh.publish(&topic, b"post-takeover", QoS::AtLeastOnce, false).await?;
            let mut ok = false;
            if let Some(m) = fresh.recv_message_timeout(RECV_TIMEOUT).await {
                ok = m.payload.as_ref() == b"post-takeover";
            }
            fresh.disconnect().await?;
            anyhow::ensure!(ok, "post-takeover pub/sub failed");
            Ok(())
        });
        verdict(self.name(), start, result)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(40)
    }
}

/// A crowd of mixed-version clients: 16 subscribers (v3 / v3.1.1 / v5)
/// against a shared topic, fed by 3 publishers of different versions.
pub struct CompatManyMixedClientsTest;

impl TestCase for CompatManyMixedClientsTest {
    fn name(&self) -> &str {
        "compat_many_mixed_clients"
    }

    fn execute(&self, ctx: &mut TestContext) -> TestResult {
        let start = Instant::now();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result: anyhow::Result<()> = rt.block_on(async {
            let addr = ctx.config.broker_addr.clone();
            let group = format!("compat/group/{}", unique_tag());
            const WANT: usize = 12;

            let mut recv_tasks = Vec::new();

            for i in 0..6u8 {
                let mut c = MqttV3Client::connect(&addr, &unique_id("crowd-v3"), CONN_TIMEOUT).await?;
                c.subscribe(&group).await?;
                recv_tasks.push(tokio::spawn(async move {
                    let mut count = 0usize;
                    while count < WANT {
                        match c.recv_message_timeout(Duration::from_secs(10)).await {
                            Some(_) => count += 1,
                            None => break,
                        }
                    }
                    c.disconnect().await.ok();
                    (format!("v3-{i}"), count)
                }));
            }
            for i in 0..5u8 {
                let mut c =
                    MqttV311Client::connect(&addr, &unique_id("crowd-v311"), CONN_TIMEOUT).await?;
                c.subscribe(&group, QoS::AtLeastOnce).await?;
                recv_tasks.push(tokio::spawn(async move {
                    let mut count = 0usize;
                    while count < WANT {
                        match c.recv_message_timeout(Duration::from_secs(10)).await {
                            Some(_) => count += 1,
                            None => break,
                        }
                    }
                    c.disconnect().await.ok();
                    (format!("v311-{i}"), count)
                }));
            }
            for i in 0..5u8 {
                let mut c = MqttV5Client::connect(&addr, &unique_id("crowd-v5"), CONN_TIMEOUT).await?;
                c.subscribe(&group, QoS::AtLeastOnce).await?;
                recv_tasks.push(tokio::spawn(async move {
                    let mut count = 0usize;
                    while count < WANT {
                        match c.recv_message_timeout(Duration::from_secs(10)).await {
                            Some(_) => count += 1,
                            None => break,
                        }
                    }
                    c.disconnect().await.ok();
                    (format!("v5-{i}"), count)
                }));
            }
            tokio::time::sleep(Duration::from_millis(300)).await;

            let pub311 = MqttV311Client::connect(&addr, &unique_id("crowd-pub311"), CONN_TIMEOUT).await?;
            let pub5 = MqttV5Client::connect(&addr, &unique_id("crowd-pub5"), CONN_TIMEOUT).await?;
            let pub3 = MqttV3Client::connect(&addr, &unique_id("crowd-pub3"), CONN_TIMEOUT).await?;
            for i in 0..4 {
                pub311.publish(&group, format!("p311-{i}").as_bytes(), QoS::AtLeastOnce, false).await?;
                pub5.publish(&group, format!("p5-{i}").as_bytes(), QoS::AtLeastOnce, false).await?;
                pub3.publish(&group, format!("p3-{i}").as_bytes()).await?;
            }
            pub311.disconnect().await?;
            pub5.disconnect().await?;
            pub3.disconnect().await?;

            let mut reports = Vec::new();
            for t in recv_tasks {
                reports.push(t.await.expect("recv task panicked"));
            }
            for (who, count) in &reports {
                anyhow::ensure!(*count == WANT, "crowd subscriber {who} received {count}/{WANT} messages");
            }
            Ok(())
        });
        verdict(self.name(), start, result)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(90)
    }
}
