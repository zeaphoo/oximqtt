//! Built-in system topic module.
//!
//! Publishes broker system status and metrics to `$SYS/` topics.

use std::convert::From as _;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use serde::{
    de::{self, Deserializer},
    Deserialize, Serialize,
};
use serde_json::json;
use tokio::{spawn, sync::RwLock, time::sleep};

use crate::codec::v5::PublishProperties;
use crate::context::ServerContext;
use crate::hook::{Handler, HookResult, Parameter, ReturnType, Type};
use crate::session::SessionState;
use crate::types::{CodecPublish, From, Id, Publish, QoS};
use crate::types::{ClientId, NodeId, TopicName, UserName};
use crate::utils::{deserialize_duration, timestamp_millis, to_duration};
use crate::Result;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginConfig {
    #[serde(
        default = "PluginConfig::publish_qos_default",
        deserialize_with = "PluginConfig::deserialize_publish_qos"
    )]
    pub publish_qos: QoS,

    #[serde(
        default = "PluginConfig::publish_interval_default",
        deserialize_with = "PluginConfig::deserialize_publish_interval"
    )]
    pub publish_interval: Duration,

    #[serde(default = "PluginConfig::message_retain_available_default")]
    pub message_retain_available: bool,

    #[serde(
        default = "PluginConfig::message_expiry_interval_default",
        deserialize_with = "deserialize_duration"
    )]
    pub message_expiry_interval: Duration,
}

impl PluginConfig {
    fn publish_qos_default() -> QoS {
        QoS::AtLeastOnce
    }

    fn publish_interval_default() -> Duration {
        Duration::from_secs(60)
    }

    fn message_retain_available_default() -> bool {
        false
    }

    fn message_expiry_interval_default() -> Duration {
        Duration::from_secs(300)
    }

    fn deserialize_publish_qos<'de, D>(deserializer: D) -> std::result::Result<QoS, D::Error>
    where
        D: Deserializer<'de>,
    {
        let qos = match u8::deserialize(deserializer)? {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => return Err(de::Error::custom("QoS configuration error, only values (0,1,2) are supported")),
        };
        Ok(qos)
    }

    pub fn deserialize_publish_interval<'de, D>(deserializer: D) -> std::result::Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = String::deserialize(deserializer)?;
        let d = to_duration(&v);
        if d < Duration::from_secs(1) {
            Err(de::Error::custom("'publish_interval' must be greater than 1 second"))
        } else {
            Ok(d)
        }
    }
}

pub async fn init(scx: &ServerContext) -> Result<()> {
    let cfg = {
        let val = crate::conf::Settings::instance().sys_topic.clone();
        let val = if val.is_null() { serde_json::json!({}) } else { val };
        serde_json::from_value::<PluginConfig>(val)?
    };
    log::info!("sys_topic cfg: {cfg:?}");
    let register = scx.extends.hook_mgr().register();
    let cfg = Arc::new(RwLock::new(cfg));
    let running = Arc::new(AtomicBool::new(true));

    register.add(Type::SessionCreated, Box::new(SystemTopicHandler::new(scx, &cfg))).await;
    register.add(Type::SessionTerminated, Box::new(SystemTopicHandler::new(scx, &cfg))).await;
    register.add(Type::ClientConnected, Box::new(SystemTopicHandler::new(scx, &cfg))).await;
    register.add(Type::ClientDisconnected, Box::new(SystemTopicHandler::new(scx, &cfg))).await;
    register.add(Type::SessionSubscribed, Box::new(SystemTopicHandler::new(scx, &cfg))).await;
    register.add(Type::SessionUnsubscribed, Box::new(SystemTopicHandler::new(scx, &cfg))).await;
    register.start().await;

    let scx_bg = scx.clone();
    let cfg_bg = cfg.clone();
    let running_bg = running.clone();
    spawn(async move {
        let min = Duration::from_secs(1);
        loop {
            let (publish_interval, publish_qos, expiry_interval) = {
                let cfg_rl = cfg_bg.read().await;
                (cfg_rl.publish_interval, cfg_rl.publish_qos, cfg_rl.message_expiry_interval)
            };

            let publish_interval = if publish_interval < min { min } else { publish_interval };
            sleep(publish_interval).await;
            if running_bg.load(Ordering::SeqCst) {
                send_stats(&scx_bg, publish_qos, expiry_interval).await;
                send_metrics(&scx_bg, publish_qos, expiry_interval).await;
            }
        }
    });

    Ok(())
}

async fn send_stats(scx: &ServerContext, publish_qos: QoS, expiry_interval: Duration) {
    let payload = scx.stats.clone(scx).await.to_json(scx).await;
    let nodeid = scx.node.id();
    let topic = format!("$SYS/brokers/{nodeid}/stats");
    sys_publish(scx.clone(), nodeid, topic, publish_qos, payload, expiry_interval).await;
}

async fn send_metrics(scx: &ServerContext, publish_qos: QoS, expiry_interval: Duration) {
    let payload = scx.metrics.to_json();
    let nodeid = scx.node.id();
    let topic = format!("$SYS/brokers/{nodeid}/metrics");
    sys_publish(scx.clone(), nodeid, topic, publish_qos, payload, expiry_interval).await;
}

struct SystemTopicHandler {
    scx: ServerContext,
    cfg: Arc<RwLock<PluginConfig>>,
    nodeid: NodeId,
}

impl SystemTopicHandler {
    fn new(scx: &ServerContext, cfg: &Arc<RwLock<PluginConfig>>) -> Self {
        Self { scx: scx.clone(), cfg: cfg.clone(), nodeid: scx.node.id() }
    }
}

#[async_trait]
impl Handler for SystemTopicHandler {
    async fn hook(&self, param: &Parameter, acc: Option<HookResult>) -> ReturnType {
        log::debug!("param: {param:?}, acc: {acc:?}");
        let now = chrono::Local::now();
        let now_time = now.format("%Y-%m-%d %H:%M:%S%.3f").to_string();
        if let Some((topic, payload)) = match param {
            Parameter::SessionCreated(session) => {
                let body = json!({
                    "node": session.id.node(),
                    "ipaddress": session.id.remote_addr,
                    "clientid": session.id.client_id,
                    "username": session.id.username_ref(),
                    "created_at": session.created_at().await.unwrap_or_default(),
                    "time": now_time
                });
                let topic = format!("$SYS/brokers/{}/session/{}/created", self.nodeid, session.id.client_id);
                Some((topic, body))
            }

            Parameter::SessionTerminated(session, reason) => {
                let body = json!({
                    "node": session.id.node(),
                    "ipaddress": session.id.remote_addr,
                    "clientid": session.id.client_id,
                    "username": session.id.username_ref(),
                    "reason": reason.to_string(),
                    "time": now_time
                });
                let topic =
                    format!("$SYS/brokers/{}/session/{}/terminated", self.nodeid, session.id.client_id);
                Some((topic, body))
            }
            Parameter::ClientConnected(session) => {
                let mut body = session
                    .connect_info()
                    .await
                    .map(|connect_info| connect_info.to_hook_body(true))
                    .unwrap_or_default();
                if let Some(obj) = body.as_object_mut() {
                    obj.insert(
                        "connected_at".into(),
                        serde_json::Value::Number(serde_json::Number::from(
                            session.connected_at().await.unwrap_or_default(),
                        )),
                    );
                    obj.insert(
                        "session_present".into(),
                        serde_json::Value::Bool(session.session_present().await.unwrap_or_default()),
                    );
                    obj.insert("time".into(), serde_json::Value::String(now_time));
                }
                let topic =
                    format!("$SYS/brokers/{}/clients/{}/connected", self.nodeid, session.id.client_id);
                Some((topic, body))
            }
            Parameter::ClientDisconnected(session, reason) => {
                let body = json!({
                    "node": session.id.node(),
                    "ipaddress": session.id.remote_addr,
                    "clientid": session.id.client_id,
                    "username": session.id.username_ref(),
                    "disconnected_at": session.disconnected_at().await.unwrap_or_default(),
                    "reason": reason.to_string(),
                    "time": now_time
                });
                let topic =
                    format!("$SYS/brokers/{}/clients/{}/disconnected", self.nodeid, session.id.client_id);
                Some((topic, body))
            }

            Parameter::SessionSubscribed(session, subscribe) => {
                let body = json!({
                    "node": session.id.node(),
                    "ipaddress": session.id.remote_addr,
                    "clientid": session.id.client_id,
                    "username": session.id.username_ref(),
                    "topic": subscribe.topic_filter,
                    "opts": subscribe.opts.to_json(),
                    "time": now_time
                });
                let topic =
                    format!("$SYS/brokers/{}/session/{}/subscribed", self.nodeid, session.id.client_id);
                Some((topic, body))
            }

            Parameter::SessionUnsubscribed(session, unsubscribed) => {
                let topic = unsubscribed.topic_filter.clone();
                let body = json!({
                    "node": session.id.node(),
                    "ipaddress": session.id.remote_addr,
                    "clientid": session.id.client_id,
                    "username": session.id.username_ref(),
                    "topic": topic,
                    "time": now_time
                });
                let topic =
                    format!("$SYS/brokers/{}/session/{}/unsubscribed", self.nodeid, session.id.client_id);
                Some((topic, body))
            }

            _ => {
                log::error!("unimplemented, {param:?}");
                None
            }
        } {
            let nodeid = self.nodeid;
            let (publish_qos, expiry_interval) = {
                let cfg_rl = self.cfg.read().await;
                (cfg_rl.publish_qos, cfg_rl.message_expiry_interval)
            };

            let scx = self.scx.clone();
            spawn(sys_publish(scx, nodeid, topic, publish_qos, payload, expiry_interval));
        }
        (true, acc)
    }
}

#[inline]
async fn sys_publish(
    scx: ServerContext,
    nodeid: NodeId,
    topic: String,
    publish_qos: QoS,
    payload: serde_json::Value,
    message_expiry_interval: Duration,
) {
    match serde_json::to_string(&payload) {
        Ok(payload) => {
            let from = From::from_system(Id::new(
                nodeid,
                0,
                None,
                None,
                ClientId::from_static("system"),
                Some(UserName::from("system")),
            ));

            let p = CodecPublish {
                dup: false,
                retain: false,
                qos: publish_qos,
                topic: TopicName::from(topic),
                packet_id: None,
                payload: Bytes::from(payload),
                properties: Some(PublishProperties::default()),
            };
            let p = <CodecPublish as Into<Publish>>::into(p).create_time(timestamp_millis());

            let p = scx.extends.hook_mgr().message_publish(None, from.clone(), &p).await.unwrap_or(p);

            let storage_available = scx.extends.message_mgr().await.enable();

            if let Err(e) =
                SessionState::forwards(&scx, from, p, storage_available, Some(message_expiry_interval)).await
            {
                log::warn!("{e}");
            }
        }
        Err(e) => {
            log::error!("{e}");
        }
    }
}
