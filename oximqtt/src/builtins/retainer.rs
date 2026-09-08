//! Built-in retained message storage module.
//!
//! Provides in-memory storage for MQTT retained messages with
//! configurable limits on count, payload size, and TTL.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::retain::{DefaultRetainStorage, RetainStorage};
use crate::types::{Retain, TopicFilter, TopicName};
use crate::utils::{deserialize_duration_option, Bytesize, StatsMergeMode};
use crate::Result;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginConfig {
    #[serde(default = "PluginConfig::max_retained_messages_default")]
    pub max_retained_messages: isize,

    #[serde(default = "PluginConfig::max_payload_size_default")]
    pub max_payload_size: Bytesize,

    #[serde(default, deserialize_with = "deserialize_duration_option")]
    pub retained_message_ttl: Option<Duration>,
}

impl PluginConfig {
    /// Bounded by default: the store is RAM-based, so an unbounded retained
    /// message count is a memory-exhaustion hazard if left to user config.
    /// `max_retained_messages` counts distinct topics holding a retained
    /// message. Set `0` explicitly to opt out of the limit.
    fn max_retained_messages_default() -> isize {
        10_000
    }

    fn max_payload_size_default() -> Bytesize {
        Bytesize::from(1024 * 1024)
    }
}

#[derive(Clone)]
struct RamRetainer {
    inner: Arc<DefaultRetainStorage>,
    cfg: Arc<RwLock<PluginConfig>>,
}

impl RamRetainer {
    fn new(cfg: Arc<RwLock<PluginConfig>>) -> RamRetainer {
        Self { inner: Arc::new(DefaultRetainStorage::new()), cfg }
    }

    async fn remove_expired_messages(&self) -> usize {
        self.inner.remove_expired_messages().await
    }
}

#[async_trait]
impl RetainStorage for RamRetainer {
    #[inline]
    fn enable(&self) -> bool {
        true
    }

    #[inline]
    fn merge_on_read(&self) -> bool {
        false
    }

    #[inline]
    fn need_sync(&self) -> bool {
        true
    }

    async fn set(&self, topic: &TopicName, retain: Retain, expiry_interval: Option<Duration>) -> Result<()> {
        let (max_retained_messages, max_payload_size, retained_message_ttl) = {
            let cfg = self.cfg.read().await;
            (cfg.max_retained_messages, *cfg.max_payload_size, cfg.retained_message_ttl)
        };

        if retain.publish.payload.len() > max_payload_size {
            log::warn!("Retain message payload exceeding limit, topic: {topic:?}, retain: {retain:?}");
            return Ok(());
        }

        if max_retained_messages > 0 && self.inner.count().await >= max_retained_messages {
            // The cap counts distinct topics. Replacing an already-retained
            // topic consumes no new slot and must stay allowed, otherwise
            // hitting the cap would silently freeze every existing retained
            // state. Only brand-new topics are refused.
            let existing = !self.get(topic).await?.is_empty();
            if !existing {
                log::warn!(
                    "The retained message has exceeded the maximum limit of: {max_retained_messages}, refuse new topic: {topic:?}"
                );
                return Ok(());
            }
        }

        let expiry_interval = retained_message_ttl
            .map(|ttl| if ttl.is_zero() { None } else { Some(ttl) })
            .unwrap_or(expiry_interval);

        self.inner.set_with_timeout(topic, retain, expiry_interval).await
    }

    async fn get(&self, topic_filter: &TopicFilter) -> Result<Vec<(TopicName, Retain)>> {
        Ok(self.inner.get_message(topic_filter).await?)
    }

    async fn get_all_paginated(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<(TopicName, Retain, Option<Duration>)>, bool)> {
        self.inner.get_all_paginated(offset, limit).await
    }

    #[inline]
    async fn count(&self) -> isize {
        self.inner.count().await
    }

    #[inline]
    async fn max(&self) -> isize {
        self.inner.max().await
    }

    #[inline]
    fn stats_merge_mode(&self) -> StatsMergeMode {
        StatsMergeMode::Max
    }
}

pub async fn init(scx: &crate::context::ServerContext) -> Result<()> {
    let cfg = {
        let val = crate::conf::Settings::instance().retainer.clone();
        let val = if val.is_null() { serde_json::json!({}) } else { val };
        serde_json::from_value::<PluginConfig>(val)?
    };
    log::info!("retainer cfg: {cfg:?}");
    let cfg = Arc::new(RwLock::new(cfg));

    let retainer = RamRetainer::new(cfg.clone());

    let retainer_bg = retainer.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let removeds = retainer_bg.remove_expired_messages().await;
            if removeds > 0 {
                log::debug!(
                    "{:?} remove_expired_messages, removed count: {}",
                    std::thread::current().id(),
                    removeds
                );
            }
        }
    });

    let r: Box<dyn RetainStorage> = Box::new(retainer);
    *scx.extends.retain_mut().await = r;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An empty `[retainer]` section must parse and fall back to safe,
    /// bounded defaults (regression guard for missing `serde(default)`s).
    #[test]
    fn empty_section_uses_bounded_defaults() {
        let cfg: PluginConfig = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(cfg.max_retained_messages > 0, "default retained cap must be bounded");
        assert_eq!(cfg.max_retained_messages, 10_000);
        assert_eq!(cfg.max_payload_size.0, 1024 * 1024);
        assert_eq!(cfg.retained_message_ttl, None);
    }

    #[test]
    fn explicit_values_override_defaults() {
        let cfg: PluginConfig = serde_json::from_value(serde_json::json!({
            "max_retained_messages": 0,
            "max_payload_size": "64KB",
            "retained_message_ttl": "1h"
        }))
        .unwrap();
        assert_eq!(cfg.max_retained_messages, 0); // explicit opt-out of the cap
        assert_eq!(cfg.max_payload_size.0, 64 * 1024);
        assert_eq!(cfg.retained_message_ttl, Some(Duration::from_secs(3600)));
    }
}
