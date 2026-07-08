//! Built-in ACL (Access Control List) module.
//!
//! Provides rule-based publish/subscribe authorization using
//! configurable allow/deny rules with topic pattern matching.

use std::str::FromStr;
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use serde::de::{self, Deserializer};
use serde::ser;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::codec::v5::SubscribeAckReason;
use crate::context::ServerContext;
use crate::hook::{Handler, HookResult, Parameter, Priority, ReturnType, Type};
use crate::trie::{TopicTree, VecToString, VecToTopic};
use crate::types::{
    AuthResult, ClientId, Id, Password, PublishAclResult, SubscribeAclResult, Superuser, Topic, UserName,
};
use crate::Result;

type DashSet<V> = dashmap::DashSet<V, ahash::RandomState>;

const PH_C: &str = "%c";
const PH_U: &str = "%u";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginConfig {
    #[serde(default = "PluginConfig::disconnect_if_pub_rejected_default")]
    pub disconnect_if_pub_rejected: bool,

    #[serde(default = "PluginConfig::priority_default")]
    pub priority: Priority,

    #[serde(
        default = "PluginConfig::rules_default",
        serialize_with = "PluginConfig::serialize_rules",
        deserialize_with = "PluginConfig::deserialize_rules"
    )]
    rules: (Vec<Rule>, serde_json::Value),
}

impl PluginConfig {
    fn disconnect_if_pub_rejected_default() -> bool {
        true
    }

    fn priority_default() -> Priority {
        10
    }

    fn rules_default() -> (Vec<Rule>, serde_json::Value) {
        let rules = r###"rules = [
                ["allow", { user = "dashboard" }, "subscribe", ["$SYS/#"]],
                ["allow", { ipaddr = "127.0.0.1" }, "pubsub", ["$SYS/#", "#"]],
                ["deny", "all", "subscribe", ["$SYS/#", { eq = "#" }]],
                ["allow", "all"]
        ]"###;

        let josn_rules = match toml::from_str::<serde_json::Value>(rules) {
            Ok(mut josn_rules) => {
                let rules =
                    josn_rules.as_object_mut().and_then(|obj| obj.remove("rules").map(Self::parse_rules));
                match rules {
                    Some(Ok(rules)) => rules,
                    Some(Err(e)) => {
                        log::error!("{e}");
                        Default::default()
                    }
                    None => Default::default(),
                }
            }
            Err(e) => {
                log::error!("{e}");
                Default::default()
            }
        };

        josn_rules
    }

    #[inline]
    pub fn rules(&self) -> &Vec<Rule> {
        let (_rules, _) = &self.rules;
        _rules
    }

    #[inline]
    fn serialize_rules<S>(
        rules: &(Vec<Rule>, serde_json::Value),
        s: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let (_, rules) = rules;
        rules.serialize(s)
    }

    #[inline]
    pub fn deserialize_rules<'de, D>(
        deserializer: D,
    ) -> std::result::Result<(Vec<Rule>, serde_json::Value), D::Error>
    where
        D: Deserializer<'de>,
    {
        let json_rules = serde_json::Value::deserialize(deserializer)?;
        Self::parse_rules(json_rules).map_err(de::Error::custom)
    }

    #[inline]
    fn parse_rules(json_rules: serde_json::Value) -> Result<(Vec<Rule>, serde_json::Value)> {
        let mut rules = Vec::new();
        if let Some(rules_cfg) = json_rules.as_array() {
            for rule_cfg in rules_cfg {
                let r = Rule::try_from(rule_cfg)?;
                rules.push(r);
            }
        }
        Ok((rules, json_rules))
    }
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub access: Access,
    pub users: Vec<User>,
    pub control: Control,
    pub topics: Topics,
}

impl Rule {
    #[inline]
    pub async fn add_topic_filter(&self, topic_filter: &str, clientid: ClientId) -> Result<()> {
        let t = Topic::from_str(topic_filter)?;
        self.topics.tree.write().await.insert(&t, Some(clientid));
        Ok(())
    }

    #[inline]
    pub async fn remove_topic(&self, topic: &str, clientid: &str) -> Result<()> {
        let mut topics = Vec::new();
        {
            let t = Topic::from_str(topic)?;
            for (topic_levels, clientids) in self.topics.tree.read().await.matches(&t).iter() {
                for cid in clientids.iter().copied().flatten() {
                    if *cid == clientid {
                        topics.push(topic_levels.to_topic());
                    }
                }
            }
        }
        let clientid = Some(ClientId::from(clientid));
        for topic in topics {
            self.topics.tree.write().await.remove(&topic, &clientid);
        }
        Ok(())
    }

    #[inline]
    pub fn add_topic_to_eqs(&self, topic: String) {
        self.topics.eqs.insert(topic);
    }

    #[inline]
    pub fn hit(
        &self,
        id: &Id,
        password: Option<&Password>,
        protocol: Option<u8>,
        allow: bool,
    ) -> (bool, Superuser) {
        let mut superuser: Superuser = false;
        for user in &self.users {
            let (hit, _superuser) = user.hit(id, password, protocol, allow);
            if !hit {
                return (false, false);
            }
            superuser = _superuser;
        }
        (true, superuser)
    }
}

impl std::convert::TryFrom<&serde_json::Value> for Rule {
    type Error = crate::Error;
    #[inline]
    fn try_from(rule_cfg: &serde_json::Value) -> std::result::Result<Self, Self::Error> {
        let err_msg = format!("ACL Rule config error, rule config is {rule_cfg:?}");
        if let Some(cfg_items) = rule_cfg.as_array() {
            let access_cfg = cfg_items.first().ok_or_else(|| anyhow!(err_msg.clone()))?;
            let user_cfg = cfg_items.get(1).ok_or_else(|| anyhow!(err_msg))?;
            let control_cfg = cfg_items.get(2);
            let topics_cfg = cfg_items.get(3);

            let access = Access::try_from(access_cfg)?;
            let users = users_try_from(user_cfg, access)?;
            let control = Control::try_from(control_cfg)?;
            let topics = Topics::try_from(topics_cfg)?;
            if topics_cfg.is_some() && matches!(control, Control::Connect) {
                log::warn!("ACL Rule config, the third column of a quadruple is Connect, but the fourth column is not empty! topics config is {topics_cfg:?}");
            }
            Ok(Rule { access, users, control, topics })
        } else {
            Err(anyhow!(err_msg))
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Access {
    Allow,
    Deny,
}

#[derive(Debug, Clone)]
pub enum User {
    Username(UserName, Option<Password>, Superuser),
    Clientid(ClientId),
    Ipaddr(String),
    Protocol(u8),
    All,
}

impl User {
    #[inline]
    pub fn hit(
        &self,
        id: &Id,
        password: Option<&Password>,
        protocol: Option<u8>,
        allow: bool,
    ) -> (bool, Superuser) {
        match self {
            User::All => (true, false),
            User::Username(name1, password1, superuser) => {
                match (id.username.as_ref(), password, password1, allow) {
                    (Some(name2), Some(password2), Some(password1), true) => {
                        (name1 == name2 && password1 == password2, *superuser)
                    }
                    (Some(name2), Some(_), &Some(_), false) => (name1 == name2, false),
                    (Some(name2), _, None, true) => (name1 == name2, *superuser),
                    (Some(name2), _, None, false) => (name1 == name2, false),
                    (Some(_), None, Some(_), _) => (false, false),
                    (None, _, _, _) => (false, false),
                }
            }
            User::Clientid(clientid) => (id.client_id == clientid, false),
            User::Ipaddr(ipaddr) => {
                if let Some(remote_addr) = id.remote_addr {
                    (ipaddr == remote_addr.ip().to_string().as_str(), false)
                } else {
                    (false, false)
                }
            }
            User::Protocol(protocol1) => {
                if let Some(protocol) = protocol {
                    (protocol == *protocol1, false)
                } else {
                    (false, false)
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Control {
    All,
    Connect,
    Publish,
    Subscribe,
    Pubsub,
}

#[derive(Debug, Clone)]
pub struct Topics {
    pub all: bool,
    pub eqs: Arc<DashSet<String>>,
    pub eq_placeholders: Vec<String>,
    pub tree: Arc<RwLock<TopicTree<Option<ClientId>>>>,
    pub placeholders: Vec<String>,
}

impl Topics {
    pub async fn is_match(&self, topic_filter: &Topic, topic_filter_str: &str, client_id: &str) -> bool {
        if self.all {
            return true;
        }

        if self.eqs.contains(topic_filter_str) {
            return true;
        }

        {
            let tree = self.tree.read().await;
            let matcheds = tree.matches(topic_filter);
            for (topic, values) in matcheds.iter() {
                log::debug!("topic: {:?}, topic_filter_str: {topic_filter_str}", topic.to_string());
                log::debug!("values: {values:?}");
                for cid in values {
                    log::debug!("cid: {cid:?}, client_id: {client_id:?}");
                    if let Some(cid) = cid {
                        if cid == client_id {
                            return true;
                        }
                    } else {
                        return true;
                    }
                }
            }
        }
        false
    }
}

impl std::convert::TryFrom<&serde_json::Value> for Access {
    type Error = crate::Error;
    #[inline]
    fn try_from(access_cfg: &serde_json::Value) -> std::result::Result<Self, Self::Error> {
        let err_msg = format!("ACL Rule config error, access config is {access_cfg:?}");
        match access_cfg.as_str().ok_or_else(|| anyhow!(err_msg.clone()))?.to_lowercase().as_str() {
            "allow" => Ok(Access::Allow),
            "deny" => Ok(Access::Deny),
            _ => Err(anyhow!(err_msg)),
        }
    }
}

fn users_try_from(user_cfg: &serde_json::Value, access: Access) -> Result<Vec<User>> {
    let err_msg = format!("ACL Rule config error, user config is {user_cfg:?}");
    let users = match user_cfg {
        serde_json::Value::String(all) => {
            if all.to_lowercase() == "all" {
                Ok(vec![User::All])
            } else {
                Err(anyhow!(err_msg))
            }
        }
        serde_json::Value::Object(map) => {
            let name = map.get("user").and_then(|v| v.as_str());
            let password = map.get("password");
            let superuser = map.get("superuser").and_then(|v| v.as_bool());
            let clientid = map.get("clientid").and_then(|v| v.as_str());
            let ipaddr = map.get("ipaddr").and_then(|v| v.as_str());
            let mqtt_protocol = map.get("protocol").and_then(|v| v.as_u64());

            let mut users = Vec::new();
            if let Some(name) = name {
                match access {
                    Access::Allow => {
                        let password = match password {
                            Some(serde_json::Value::String(p)) => Some(Password::from(p.to_owned())),
                            None => None,
                            _ => return Err(anyhow!(err_msg)),
                        };
                        let superuser = superuser.unwrap_or_default();
                        users.push(User::Username(UserName::from(name), password, superuser));
                    }
                    Access::Deny => {
                        users.push(User::Username(UserName::from(name), None, false));
                    }
                }
            }

            if let Some(clientid) = clientid {
                users.push(User::Clientid(ClientId::from(clientid)));
            }

            if let Some(ipaddr) = ipaddr {
                users.push(User::Ipaddr(String::from(ipaddr)));
            }

            if let Some(mqtt_protocol) = mqtt_protocol {
                users.push(User::Protocol(mqtt_protocol as u8));
            }
            Ok(users)
        }
        _ => Err(anyhow!(err_msg)),
    };
    users
}

impl std::convert::TryFrom<Option<&serde_json::Value>> for Control {
    type Error = crate::Error;
    #[inline]
    fn try_from(control_cfg: Option<&serde_json::Value>) -> std::result::Result<Self, Self::Error> {
        let err_msg = format!("ACL Rule config error, control config is {control_cfg:?}");
        let control = match control_cfg {
            None => Ok(Control::All),
            Some(serde_json::Value::String(control)) => match control.to_lowercase().as_str() {
                "connect" => Ok(Control::Connect),
                "publish" => Ok(Control::Publish),
                "subscribe" => Ok(Control::Subscribe),
                "pubsub" => Ok(Control::Pubsub),
                "all" => Ok(Control::All),
                _ => Err(anyhow!(err_msg)),
            },
            _ => Err(anyhow!(err_msg)),
        };
        control
    }
}

impl std::convert::TryFrom<Option<&serde_json::Value>> for Topics {
    type Error = crate::Error;
    #[inline]
    fn try_from(topics_cfg: Option<&serde_json::Value>) -> std::result::Result<Self, Self::Error> {
        let err_msg = format!("ACL Rule config error, topics config is {topics_cfg:?}");
        let mut all = false;
        let eqs = DashSet::default();
        let mut tree = TopicTree::default();
        let mut placeholders = Vec::new();
        let mut eq_placeholders = Vec::new();
        match topics_cfg {
            None => all = true,
            Some(serde_json::Value::Array(topics)) => {
                for topic in topics.iter() {
                    match topic {
                        serde_json::Value::String(topic) => {
                            if topic.contains(PH_U) || topic.contains(PH_C) {
                                placeholders.push(topic.clone());
                            } else {
                                tree.insert(&Topic::from_str(topic.as_str())?, None);
                            }
                        }
                        serde_json::Value::Object(eq_map) => match eq_map.get("eq") {
                            Some(serde_json::Value::String(eq)) => {
                                if eq.contains(PH_U) || eq.contains(PH_C) {
                                    eq_placeholders.push(eq.clone());
                                } else {
                                    eqs.insert(eq.clone());
                                }
                            }
                            _ => return Err(anyhow!(err_msg)),
                        },
                        _ => return Err(anyhow!(err_msg)),
                    }
                }
            }
            _ => return Err(anyhow!(err_msg)),
        }
        Ok(Topics {
            all,
            eqs: Arc::new(eqs),
            eq_placeholders,
            tree: Arc::new(RwLock::new(tree)),
            placeholders,
        })
    }
}

const CACHE_KEY: &str = "$SYS/ACL-CACHE-MAP";

pub async fn init(scx: &ServerContext) -> Result<()> {
    let cfg = {
        let val = crate::conf::Settings::instance().acl.clone();
        let val = if val.is_null() { serde_json::json!({}) } else { val };
        serde_json::from_value::<PluginConfig>(val)?
    };
    log::info!("acl cfg: {cfg:?}");
    let cfg = Arc::new(RwLock::new(cfg));
    let register = scx.extends.hook_mgr().register();

    let priority = cfg.read().await.priority;
    register.add_priority(Type::ClientConnected, priority, Box::new(AclHandler::new(&cfg))).await;
    register.add_priority(Type::ClientDisconnected, priority, Box::new(AclHandler::new(&cfg))).await;
    register.add_priority(Type::ClientAuthenticate, priority, Box::new(AclHandler::new(&cfg))).await;
    register.add_priority(Type::ClientSubscribeCheckAcl, priority, Box::new(AclHandler::new(&cfg))).await;
    register.add_priority(Type::MessagePublishCheckAcl, priority, Box::new(AclHandler::new(&cfg))).await;
    register.start().await;

    Ok(())
}

struct AclHandler {
    cfg: Arc<RwLock<PluginConfig>>,
}

impl AclHandler {
    fn new(cfg: &Arc<RwLock<PluginConfig>>) -> Self {
        Self { cfg: cfg.clone() }
    }
}

#[async_trait]
impl Handler for AclHandler {
    async fn hook(&self, param: &Parameter, acc: Option<HookResult>) -> ReturnType {
        match param {
            Parameter::ClientConnected(session) => {
                let cfg = self.cfg.clone();
                let client_id = session.id.client_id.clone();
                let username = session.id.username.clone();
                let extra_attrs = session.extra_attrs.clone();

                let build_placeholders = async move {
                    for rule in cfg.read().await.rules() {
                        for ph_tf in &rule.topics.placeholders {
                            let mut tf = ph_tf.replace(PH_C, &client_id);
                            if let Some(un) = &username {
                                tf = tf.replace(PH_U, un);
                            } else {
                                tf = tf.replace(PH_U, "");
                            }
                            if let Err(e) = rule.add_topic_filter(&tf, client_id.clone()).await {
                                log::error!(
                                    "acl config error, build_placeholders, add topic filter error, {e}"
                                );
                            }
                            log::debug!("topic filter: {tf}");
                            if let Some(caches) =
                                extra_attrs.write().await.get_default_mut(CACHE_KEY.into(), Vec::default)
                            {
                                caches.push(tf);
                            }
                        }

                        for eq_ph_t in &rule.topics.eq_placeholders {
                            let mut t = eq_ph_t.replace(PH_C, &client_id);
                            if let Some(un) = &username {
                                t = t.replace(PH_U, un);
                            } else {
                                t = t.replace(PH_U, "");
                            }
                            log::info!("eq topic: {t}");
                            rule.add_topic_to_eqs(t);
                        }

                        log::debug!("rule.access: {:?}", rule.access);
                        log::debug!("rule.users: {:?}", rule.users);
                        log::debug!("rule.control: {:?}", rule.control);
                        log::debug!("rule.topics.eqs: {:?}", rule.topics.eqs);
                        log::debug!("rule.topics.tree: {:?}", rule.topics.tree.read().await.list(100));
                        log::debug!("rule.topics.placeholders: {:?}", rule.topics.placeholders);
                    }
                };
                tokio::spawn(build_placeholders);
            }

            Parameter::ClientDisconnected(session, _reason) => {
                if let Some(topic_filters) = session.extra_attrs.read().await.get::<Vec<String>>(CACHE_KEY) {
                    let client_id = session.id.client_id.clone();
                    for topic_filter in topic_filters {
                        for rule in self.cfg.read().await.rules() {
                            if let Err(e) = rule.remove_topic(topic_filter.as_str(), &client_id).await {
                                log::error!("remove topic filter error, {e}");
                            }
                        }
                    }
                };
            }

            Parameter::ClientAuthenticate(connect_info) => {
                log::debug!("ClientAuthenticate acl");
                if matches!(
                    acc,
                    Some(HookResult::AuthResult(AuthResult::BadUsernameOrPassword))
                        | Some(HookResult::AuthResult(AuthResult::NotAuthorized))
                ) {
                    return (false, acc);
                }

                for rule in self.cfg.read().await.rules() {
                    if !matches!(rule.control, Control::Connect | Control::All) {
                        continue;
                    }

                    let allow = matches!(rule.access, Access::Allow);
                    let (hit, superuser) = rule.hit(
                        connect_info.id(),
                        connect_info.password(),
                        Some(connect_info.proto_ver()),
                        allow,
                    );
                    if hit {
                        log::debug!("{:?} ClientAuthenticate, rule: {:?}", connect_info.id(), rule);
                        return if allow {
                            (false, Some(HookResult::AuthResult(AuthResult::Allow(superuser, None))))
                        } else {
                            (false, Some(HookResult::AuthResult(AuthResult::NotAuthorized)))
                        };
                    }
                }
                return (false, Some(HookResult::AuthResult(AuthResult::NotAuthorized)));
            }

            Parameter::ClientSubscribeCheckAcl(session, subscribe) => {
                if let Some(HookResult::SubscribeAclResult(acl_result)) = &acc {
                    if acl_result.failure() {
                        return (false, acc);
                    }
                }
                let topic =
                    Topic::from_str(&subscribe.topic_filter).unwrap_or_else(|_| Topic::from(Vec::new()));
                let topic_filter = &subscribe.topic_filter;
                for (idx, rule) in self.cfg.read().await.rules().iter().enumerate() {
                    if !matches!(rule.control, Control::Subscribe | Control::Pubsub | Control::All) {
                        continue;
                    }

                    let allow = matches!(rule.access, Access::Allow);
                    let (hit, _) =
                        rule.hit(&session.id, session.password(), session.protocol().await.ok(), allow);
                    if !hit {
                        continue;
                    }
                    if !rule.topics.is_match(&topic, topic_filter, &session.id.client_id).await {
                        continue;
                    }
                    log::debug!(
                        "{:?} ClientSubscribeCheckAcl, {}, is_match ok: topic_filter: {}",
                        session.id,
                        idx,
                        topic_filter
                    );
                    return if allow {
                        (
                            false,
                            Some(HookResult::SubscribeAclResult(SubscribeAclResult::new_success(
                                subscribe.opts.qos(),
                                None,
                            ))),
                        )
                    } else {
                        (
                            false,
                            Some(HookResult::SubscribeAclResult(SubscribeAclResult::new_failure(
                                SubscribeAckReason::UnspecifiedError,
                            ))),
                        )
                    };
                }
                return (
                    false,
                    Some(HookResult::SubscribeAclResult(SubscribeAclResult::new_failure(
                        SubscribeAckReason::UnspecifiedError,
                    ))),
                );
            }

            Parameter::MessagePublishCheckAcl(session, publish) => {
                if let Some(HookResult::PublishAclResult(acl_res)) = &acc {
                    if acl_res.is_rejected() {
                        return (false, acc);
                    }
                }

                let topic_str = &publish.topic;
                let topic = Topic::from_str(topic_str).unwrap_or_else(|_| Topic::from(Vec::new()));
                let disconnect_if_pub_rejected = self.cfg.read().await.disconnect_if_pub_rejected;
                for (idx, rule) in self.cfg.read().await.rules().iter().enumerate() {
                    if !matches!(rule.control, Control::Publish | Control::Pubsub | Control::All) {
                        continue;
                    }

                    let allow = matches!(rule.access, Access::Allow);
                    let (hit, _) =
                        rule.hit(&session.id, session.password(), session.protocol().await.ok(), allow);
                    if !hit {
                        continue;
                    }
                    if !rule.topics.is_match(&topic, topic_str, &session.id.client_id).await {
                        continue;
                    }
                    log::debug!(
                        "{:?} MessagePublishCheckAcl, {}, is_match ok: topic_str: {}",
                        session.id,
                        idx,
                        topic_str
                    );
                    return if allow {
                        (false, Some(HookResult::PublishAclResult(PublishAclResult::allow())))
                    } else {
                        (
                            false,
                            Some(HookResult::PublishAclResult(PublishAclResult::rejected(
                                disconnect_if_pub_rejected,
                                None,
                            ))),
                        )
                    };
                }
                return (
                    false,
                    Some(HookResult::PublishAclResult(PublishAclResult::rejected(
                        disconnect_if_pub_rejected,
                        None,
                    ))),
                );
            }
            _ => {
                log::error!("parameter is: {param:?}");
            }
        }
        (true, acc)
    }
}
