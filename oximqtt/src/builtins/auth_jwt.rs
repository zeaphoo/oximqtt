//! Built-in JWT authentication module.
//!
//! Authenticates MQTT clients using JSON Web Tokens (JWT).

use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use async_trait::async_trait;
use itertools::Itertools;
use jsonwebtoken::{decode, DecodingKey, TokenData, Validation};
use serde::de::{self, Deserializer};
use serde::ser::{self, Serializer};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::acl::{
    AuthInfo, Rule, PLACEHOLDER_CLIENTID, PLACEHOLDER_IPADDR, PLACEHOLDER_PROTOCOL, PLACEHOLDER_USERNAME,
};
use crate::context::ServerContext;
use crate::hook::{Handler, HookResult, Parameter, Priority, ReturnType, Type};
use crate::types::{AuthResult, ConnectInfo, Disconnect, Message, Reason};
use crate::Result;

type HashMap<K, V> = std::collections::HashMap<K, V, ahash::RandomState>;

type HasPlaceholderUsername = bool;
type HasPlaceholderClientid = bool;
type HasPlaceholderIpaddr = bool;
type HasPlaceholderProtocol = bool;
type ValidateExpEnable = bool;
type ValidateNbfEnable = bool;

type ClaimItem =
    (String, HasPlaceholderUsername, HasPlaceholderClientid, HasPlaceholderIpaddr, HasPlaceholderProtocol);

#[derive(Clone, Deserialize, Serialize)]
pub struct PluginConfig {
    #[serde(default = "PluginConfig::disconnect_if_pub_rejected_default")]
    pub disconnect_if_pub_rejected: bool,

    #[serde(default = "PluginConfig::disconnect_if_expiry_default")]
    pub disconnect_if_expiry: bool,

    #[serde(default = "PluginConfig::priority_default")]
    pub priority: Priority,

    #[serde(
        default = "PluginConfig::from_default",
        serialize_with = "PluginConfig::serialize_from",
        deserialize_with = "PluginConfig::deserialize_from"
    )]
    pub from: JWTFrom,

    #[serde(
        default = "PluginConfig::encrypt_default",
        serialize_with = "PluginConfig::serialize_encrypt",
        deserialize_with = "PluginConfig::deserialize_encrypt"
    )]
    pub encrypt: JWTEncrypt,

    #[serde(default)]
    pub hmac_secret: String,
    pub hmac_base64: bool,
    #[serde(default)]
    pub public_key: String,

    #[serde(
        default,
        serialize_with = "PluginConfig::serialize_validate_claims",
        deserialize_with = "PluginConfig::deserialize_validate_claims"
    )]
    pub validate_claims: ValidateClaims,

    #[serde(skip, default = "PluginConfig::decoded_key_default")]
    pub decoded_key: DecodingKey,
}

impl fmt::Debug for PluginConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match serde_json::to_string(self) {
            Ok(cfg) => f.debug_set().entry(&cfg).finish(),
            Err(e) => f.debug_set().entry(&e).finish(),
        }
    }
}

impl PluginConfig {
    pub(crate) fn init_decoding_key(&mut self) -> Result<()> {
        match &self.encrypt {
            JWTEncrypt::HmacBased => {
                self.decoded_key = if self.hmac_base64 {
                    DecodingKey::from_base64_secret(&self.hmac_secret).map_err(anyhow::Error::new)?
                } else {
                    DecodingKey::from_secret(self.hmac_secret.as_bytes())
                };
            }
            JWTEncrypt::PublicKey => {
                self.decoded_key = if let Ok(key) =
                    DecodingKey::from_rsa_pem(&std::fs::read(&self.public_key)?)
                {
                    key
                } else if let Ok(key) = DecodingKey::from_ec_pem(&std::fs::read(&self.public_key)?) {
                    key
                } else {
                    DecodingKey::from_ed_pem(&std::fs::read(&self.public_key)?).map_err(anyhow::Error::new)?
                };
            }
        }
        Ok(())
    }

    fn decoded_key_default() -> DecodingKey {
        DecodingKey::from_secret(b"")
    }

    fn from_default() -> JWTFrom {
        JWTFrom::Password
    }

    fn encrypt_default() -> JWTEncrypt {
        JWTEncrypt::HmacBased
    }

    fn disconnect_if_pub_rejected_default() -> bool {
        true
    }

    fn priority_default() -> Priority {
        50
    }

    fn disconnect_if_expiry_default() -> bool {
        false
    }

    #[inline]
    fn serialize_encrypt<S>(enc: &JWTEncrypt, s: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let enc = match enc {
            JWTEncrypt::HmacBased => "hmac-based",
            JWTEncrypt::PublicKey => "public-key",
        };
        enc.serialize(s)
    }

    #[inline]
    fn deserialize_encrypt<'de, D>(deserializer: D) -> std::result::Result<JWTEncrypt, D::Error>
    where
        D: Deserializer<'de>,
    {
        let enc: String = String::deserialize(deserializer)?;
        let enc = match enc.as_str() {
            "hmac-based" => JWTEncrypt::HmacBased,
            "public-key" => JWTEncrypt::PublicKey,
            _ => {
                return Err(de::Error::custom(
                    "Invalid encryption method, only 'hmac-based' and 'public-key' are supported.",
                ))
            }
        };
        Ok(enc)
    }

    #[inline]
    fn serialize_from<S>(enc: &JWTFrom, s: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let enc = match enc {
            JWTFrom::Username => "username",
            JWTFrom::Password => "password",
        };
        enc.serialize(s)
    }

    #[inline]
    fn deserialize_from<'de, D>(deserializer: D) -> std::result::Result<JWTFrom, D::Error>
    where
        D: Deserializer<'de>,
    {
        let enc: String = String::deserialize(deserializer)?;
        let enc = match enc.as_str() {
            "username" => JWTFrom::Username,
            "password" => JWTFrom::Password,
            _ => {
                return Err(de::Error::custom(
                    "Invalid jwt from, only 'username' and 'password' are supported.",
                ))
            }
        };
        Ok(enc)
    }

    #[inline]
    fn serialize_validate_claims<S>(claims: &ValidateClaims, s: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        claims.validate_customs.serialize(s)
    }

    #[inline]
    fn deserialize_validate_claims<'de, D>(deserializer: D) -> std::result::Result<ValidateClaims, D::Error>
    where
        D: Deserializer<'de>,
    {
        let claims_json: serde_json::Value =
            serde_json::Value::deserialize(deserializer).map_err(de::Error::custom)?;
        let mut validate_customs = HashMap::default();
        let mut validate_exp_enable = false;
        let mut validate_nbf_enable = false;
        let mut validate_sub = None;
        let mut validate_iss = None;
        let mut validate_aud = None;
        if let Some(objs) = claims_json.as_object() {
            for (claim, val) in objs {
                let items = if let Some(arr) = val.as_array() {
                    arr.iter().map(|v| parse(claim.as_str(), v)).collect_vec()
                } else if val.as_str().is_some() {
                    vec![parse(claim.as_str(), val)]
                } else if let Some(true) = val.as_bool() {
                    vec![parse(claim.as_str(), val)]
                } else {
                    return Err(de::Error::custom(format!("invalid value, {claim}:{val}")));
                };
                for (exp_enable, nbf_enable, _) in items.iter() {
                    if *exp_enable && !validate_exp_enable {
                        validate_exp_enable = true;
                    } else if *nbf_enable && !validate_nbf_enable {
                        validate_nbf_enable = true;
                    }
                }
                let items = items.into_iter().filter_map(|(_, _, item)| item).collect_vec();
                if !items.is_empty() {
                    match claim.as_str() {
                        "sub" => validate_sub = Some(items[0].clone()),
                        "iss" => validate_iss = Some(items),
                        "aud" => validate_aud = Some(items),
                        _ => {
                            validate_customs.insert(claim.into(), items);
                        }
                    }
                }
            }
        }

        Ok(ValidateClaims {
            validate_customs,
            validate_exp_enable,
            validate_nbf_enable,
            validate_sub,
            validate_iss,
            validate_aud,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum JWTFrom {
    Username,
    Password,
}

#[derive(Debug, Clone, Copy)]
pub enum JWTEncrypt {
    HmacBased,
    PublicKey,
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct ValidateClaims {
    pub validate_customs: HashMap<String, Vec<ClaimItem>>,
    pub validate_exp_enable: ValidateExpEnable,
    pub validate_nbf_enable: ValidateNbfEnable,
    pub validate_sub: Option<ClaimItem>,
    pub validate_iss: Option<Vec<ClaimItem>>,
    pub validate_aud: Option<Vec<ClaimItem>>,
}

fn parse(name: &str, val: &serde_json::Value) -> (ValidateExpEnable, ValidateNbfEnable, Option<ClaimItem>) {
    match name {
        "exp" => {
            if val.as_bool().unwrap_or_default() {
                return (true, false, None);
            }
        }
        "nbf" => {
            if val.as_bool().unwrap_or_default() {
                return (false, true, None);
            }
        }
        _ => {
            if let Some(s) = val.as_str() {
                return (
                    false,
                    false,
                    Some((
                        s.into(),
                        s.contains(PLACEHOLDER_USERNAME),
                        s.contains(PLACEHOLDER_CLIENTID),
                        s.contains(PLACEHOLDER_IPADDR),
                        s.contains(PLACEHOLDER_PROTOCOL),
                    )),
                );
            }
        }
    }
    (false, false, None)
}

pub async fn init(scx: &ServerContext) -> Result<()> {
    let mut cfg = {
        let val = oximqtt_conf::Settings::instance().auth_jwt.clone();
        let val = if val.is_null() { serde_json::json!({}) } else { val };
        serde_json::from_value::<PluginConfig>(val)?
    };
    cfg.init_decoding_key()?;
    log::info!("auth_jwt cfg: {cfg:?}");
    let cfg = Arc::new(RwLock::new(cfg));
    let register = scx.extends.hook_mgr().register();

    let priority = cfg.read().await.priority;
    register
        .add_priority(Type::ClientAuthenticate, priority, Box::new(AuthHandler::new(scx, &cfg)))
        .await;
    register
        .add_priority(Type::ClientSubscribeCheckAcl, priority, Box::new(AuthHandler::new(scx, &cfg)))
        .await;
    register
        .add_priority(Type::MessagePublishCheckAcl, priority, Box::new(AuthHandler::new(scx, &cfg)))
        .await;
    register.add(Type::ClientKeepalive, Box::new(AuthHandler::new(scx, &cfg))).await;
    register.start().await;

    Ok(())
}

struct AuthHandler {
    scx: ServerContext,
    cfg: Arc<RwLock<PluginConfig>>,
}

impl AuthHandler {
    fn new(scx: &ServerContext, cfg: &Arc<RwLock<PluginConfig>>) -> Self {
        Self { scx: scx.clone(), cfg: cfg.clone() }
    }

    #[inline]
    async fn token<'a>(&self, connect_info: &'a ConnectInfo) -> Option<Cow<'a, str>> {
        let token = match self.cfg.read().await.from {
            JWTFrom::Username => connect_info.username().map(|u| Cow::Borrowed(u.as_ref())),
            JWTFrom::Password => connect_info.password().map(|p| String::from_utf8_lossy(p)),
        };
        token
    }

    #[inline]
    fn replaces(
        connect_info: &ConnectInfo,
        item: &str,
        p_uname: bool,
        p_cid: bool,
        p_ipaddr: bool,
        p_proto: bool,
    ) -> Result<String> {
        let mut item = if p_uname {
            if let Some(username) = connect_info.username() {
                Cow::Owned(item.replace(PLACEHOLDER_USERNAME, username))
            } else {
                return Err(anyhow!("username does not exist"));
            }
        } else {
            Cow::Borrowed(item)
        };
        if p_cid {
            item = Cow::Owned(item.replace(PLACEHOLDER_CLIENTID, connect_info.client_id()));
        }
        if p_ipaddr {
            if let Some(ipaddr) = connect_info.ipaddress() {
                item = Cow::Owned(item.replace(PLACEHOLDER_IPADDR, ipaddr.ip().to_string().as_str()));
            } else {
                return Err(anyhow!("ipaddr does not exist"));
            }
        }
        if p_proto {
            item = Cow::Owned(
                item.replace(PLACEHOLDER_PROTOCOL, itoa::Buffer::new().format(connect_info.proto_ver())),
            );
        }
        Ok(item.into())
    }

    #[inline]
    async fn standard_auth(
        &self,
        connect_info: &ConnectInfo,
        token: &str,
        validate_claims_cfg: &ValidateClaims,
    ) -> Result<TokenData<HashMap<String, serde_json::Value>>> {
        let mut required_spec_claims = HashSet::default();

        let validate_exp = validate_claims_cfg.validate_exp_enable;
        let validate_nbf = validate_claims_cfg.validate_nbf_enable;

        let mut validate_aud = false;
        let mut aud = None;
        let mut iss = None;
        let mut sub = None;

        if let Some(validate_aud_cfg) = validate_claims_cfg.validate_aud.as_ref() {
            if !validate_aud_cfg.is_empty() {
                let items = validate_aud_cfg
                    .iter()
                    .map(|(item, p_uname, p_cid, p_ipaddr, p_proto)| {
                        Self::replaces(connect_info, item, *p_uname, *p_cid, *p_ipaddr, *p_proto)
                    })
                    .collect::<Result<HashSet<String>>>()?;
                validate_aud = true;
                aud = Some(items);
                required_spec_claims.insert("aud".into());
            }
        }

        if let Some(validate_iss_cfg) = validate_claims_cfg.validate_iss.as_ref() {
            if !validate_iss_cfg.is_empty() {
                let items = validate_iss_cfg
                    .iter()
                    .map(|(item, p_uname, p_cid, p_ipaddr, p_proto)| {
                        Self::replaces(connect_info, item, *p_uname, *p_cid, *p_ipaddr, *p_proto)
                    })
                    .collect::<Result<HashSet<String>>>()?;
                iss = Some(items);
                required_spec_claims.insert("iss".into());
            }
        }

        if let Some((item, p_uname, p_cid, p_ipaddr, p_proto)) = validate_claims_cfg.validate_sub.as_ref() {
            sub = Some(Self::replaces(connect_info, item, *p_uname, *p_cid, *p_ipaddr, *p_proto)?);
            required_spec_claims.insert("sub".into());
        }

        let header = jsonwebtoken::decode_header(token).map_err(|e| anyhow!(e))?;
        log::debug!("header: {header:?}");
        let mut validation = Validation::new(header.alg);
        validation.validate_exp = validate_exp;
        validation.validate_nbf = validate_nbf;
        validation.validate_aud = validate_aud;
        validation.aud = aud;
        validation.iss = iss;
        validation.sub = sub;
        validation.required_spec_claims = required_spec_claims;

        log::debug!("validation: {validation:?}");

        let token_data = decode::<HashMap<String, serde_json::Value>>(
            token,
            &self.cfg.read().await.decoded_key,
            &validation,
        )
        .map_err(|e| anyhow!(e))?;

        Ok(token_data)
    }

    #[inline]
    fn extended_auth(
        &self,
        connect_info: &ConnectInfo,
        validate_claims_cfg: &ValidateClaims,
        token_data: &TokenData<HashMap<String, serde_json::Value>>,
    ) -> Result<()> {
        let validates = validate_claims_cfg
            .validate_customs
            .iter()
            .map(|(name, items)| {
                items
                    .iter()
                    .map(|(item, p_uname, p_cid, p_ipaddr, p_proto)| {
                        Self::replaces(connect_info, item, *p_uname, *p_cid, *p_ipaddr, *p_proto)
                    })
                    .collect::<Result<Vec<String>>>()
                    .map(|items| (name, items))
            })
            .collect::<Result<Vec<(_, _)>>>()?;

        let failed = validates.into_iter().find_map(|(name, items)| {
            let claim_item = token_data.claims.get(name).and_then(|val| val.as_str());
            let valid_res = claim_item.map(|s| items.iter().any(|item| item == s)).unwrap_or_default();
            if !valid_res {
                Some((name, items, claim_item))
            } else {
                None
            }
        });
        log::debug!("failed: {failed:?}");
        if let Some((name, expecteds, actuals)) = failed {
            Err(anyhow!(format!(
                "{} verification failed, expected value: {:?}, actual value: {:?}",
                name, expecteds, actuals
            )))
        } else {
            Ok(())
        }
    }
}

#[async_trait]
impl Handler for AuthHandler {
    async fn hook(&self, param: &Parameter, acc: Option<HookResult>) -> ReturnType {
        match param {
            Parameter::ClientAuthenticate(connect_info) => {
                log::debug!("ClientAuthenticate auth-jwt");
                if matches!(
                    acc,
                    Some(HookResult::AuthResult(AuthResult::BadUsernameOrPassword))
                        | Some(HookResult::AuthResult(AuthResult::NotAuthorized))
                ) {
                    return (false, acc);
                }

                let token = match self.token(connect_info).await {
                    Some(token) => token,
                    None => return (false, Some(HookResult::AuthResult(AuthResult::NotAuthorized))),
                };
                log::debug!("ClientAuthenticate token: {token}");

                let validate_claims_cfg = &self.cfg.read().await.validate_claims;
                let token_data =
                    match self.standard_auth(connect_info, token.as_ref(), validate_claims_cfg).await {
                        Ok(token_data) => token_data,
                        Err(e) => {
                            log::warn!("{} token:{}, error: {}", connect_info.id(), token, e);
                            return (false, Some(HookResult::AuthResult(AuthResult::NotAuthorized)));
                        }
                    };

                if let Err(e) = self.extended_auth(connect_info, validate_claims_cfg, &token_data) {
                    log::warn!("{} {}", connect_info.id(), e);
                    return (false, Some(HookResult::AuthResult(AuthResult::NotAuthorized)));
                }

                log::debug!("token_data header: {:?}", token_data.header);
                log::debug!("token_data claims: {:?}", token_data.claims);

                let superuser =
                    token_data.claims.get("superuser").and_then(|v| v.as_bool()).unwrap_or_default();

                let rules = if let Some(acls) = token_data.claims.get("acl").and_then(|acl| acl.as_array()) {
                    match acls
                        .iter()
                        .map(|acl| Rule::try_from((acl, *connect_info)))
                        .collect::<Result<Vec<Rule>>>()
                    {
                        Err(e) => {
                            log::warn!("{} {}", connect_info.id(), e);
                            return (false, Some(HookResult::AuthResult(AuthResult::NotAuthorized)));
                        }
                        Ok(rules) => rules,
                    }
                } else {
                    Vec::new()
                };
                log::debug!("rules: {rules:?}");
                let expire_at =
                    token_data.claims.get("exp").and_then(|exp| exp.as_u64().map(Duration::from_secs));
                let auth_info = AuthInfo { superuser, expire_at, rules };
                return (false, Some(HookResult::AuthResult(AuthResult::Allow(superuser, Some(auth_info)))));
            }

            Parameter::ClientSubscribeCheckAcl(session, subscribe) => {
                log::debug!("ClientSubscribeCheckAcl auth-jwt");
                if let Some(HookResult::SubscribeAclResult(acl_result)) = &acc {
                    if acl_result.failure() {
                        return (false, acc);
                    }
                }

                if let Some(auth_info) = &session.auth_info {
                    if let Some(acl_res) = auth_info.subscribe_acl(subscribe).await {
                        return acl_res;
                    }
                }
            }

            Parameter::MessagePublishCheckAcl(session, publish) => {
                log::debug!("MessagePublishCheckAcl auth-jwt");
                if let Some(HookResult::PublishAclResult(acl_res)) = &acc {
                    if acl_res.is_rejected() {
                        return (false, acc);
                    }
                }

                if let Some(auth_info) = &session.auth_info {
                    if let Some(acl_res) =
                        auth_info.publish_acl(publish, self.cfg.read().await.disconnect_if_pub_rejected).await
                    {
                        return acl_res;
                    }
                }
            }

            Parameter::ClientKeepalive(s, _) => {
                if let Some(auth) = &s.auth_info {
                    log::debug!("Keepalive auth-jwt, is_expired: {:?}", auth.is_expired());
                    if auth.is_expired() && self.cfg.read().await.disconnect_if_expiry {
                        if let Some(tx) = self.scx.extends.shared().await.entry(s.id().clone()).tx() {
                            if let Err(e) = tx.unbounded_send(Message::Closed(Reason::ConnectDisconnect(
                                Some(Disconnect::Other("JWT Auth expired".into())),
                            ))) {
                                log::warn!("{} {}", s.id(), e);
                            }
                        }
                    }
                }
            }

            _ => {
                log::error!("unimplemented, {param:?}")
            }
        }
        (true, acc)
    }
}
