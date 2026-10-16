//! Which provider and model serves each AI role, and whether it may receive
//! mail content (an endpoint off the instance's own network gets none
//! without an explicit opt-in).

use std::{
    collections::HashMap,
    fmt,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

use campbooks_kernel::config::env_source;
use figment::{
    Figment,
    providers::Serialized,
    value::{Dict, Value},
};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Deserializer, Serialize};
use url::{Host, Url};

use crate::{Error, decide::Calibration};

/// What a call is *for*. Code asks for a role, never for a vendor or model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Typed decisions about a state: which option, what score, is it true
    /// (ADR 0009). Served by a decision model (Laya) or an instruct model.
    Decide,
    /// Text embeddings for search and similarity.
    Embed,
    /// Longer free-form generation (drafts, summaries, Scout replies).
    Complete,
    /// Understanding images and scanned documents. Optional.
    Vision,
}

impl Role {
    /// Every role, in a stable order.
    pub const ALL: [Self; 4] = [Self::Decide, Self::Embed, Self::Complete, Self::Vision];

    /// Lower-case name, as used in config keys (`CAMPBOOKS_AI__<ROLE>__…`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Decide => "decide",
            Self::Embed => "embed",
            Self::Complete => "complete",
            Self::Vision => "vision",
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The wire protocol a role's provider speaks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    /// The OpenAI-compatible HTTP API (`/v1/chat/completions`,
    /// `/v1/embeddings`). The default, and the only protocol of `embed`,
    /// `complete` and `vision`. For `decide` it means an instruct model
    /// answering typed questions with JSON-schema output.
    #[default]
    #[serde(rename = "openai")]
    OpenAi,
    /// `laya-serve`'s typed decision protocol (`POST /v1/systemone`).
    /// `decide` only (ADR 0009).
    Laya,
}

impl Protocol {
    /// Lower-case name, as used in `CAMPBOOKS_AI__<ROLE>__PROTOCOL`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Laya => "laya",
        }
    }
}

/// An endpoint, the protocol it speaks and the model to use on it.
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    /// API base URL **including the version prefix**, as OpenAI SDKs expect:
    /// `http://ollama:11434/v1`, `http://tei/v1`, `https://api.example.com/v1`,
    /// `http://laya:8000/v1`. The gateway appends `chat/completions`,
    /// `embeddings` or `systemone`.
    pub base_url: Url,
    /// Bearer token, if the provider needs one. Never logged.
    #[serde(default)]
    pub api_key: Option<SecretString>,
    /// Model identifier as the provider names it (`qwen3:8b`, `BAAI/bge-m3`,
    /// `convaiinnovations/laya-multilingual`, …).
    pub model: String,
    /// Wire protocol (`CAMPBOOKS_AI__<ROLE>__PROTOCOL`, default `openai`).
    #[serde(default)]
    pub protocol: Protocol,
    /// `decide` only: per-question-type temperatures applied to answer
    /// probabilities (`CAMPBOOKS_AI__DECIDE__CALIBRATION__<TYPE>[_<OPTIONS>]`).
    #[serde(default)]
    pub calibration: Calibration,
    /// Whether this role may receive mail content when its endpoint is off
    /// the instance's network ([`Location::External`]):
    /// `CAMPBOOKS_AI__<ROLE>__EXTERNAL_CONTENT=allow`. Default `deny`: the
    /// gateway then refuses every call that carries content
    /// ([`Error::ExternalContentRefused`]). Ignored for a role on the
    /// instance's network.
    #[serde(default, deserialize_with = "external_content")]
    pub external_content: ExternalContent,
    /// `decide` over `openai` only: ask the provider for token log-probabilities,
    /// from which answer probabilities are derived
    /// (`CAMPBOOKS_AI__DECIDE__LOGPROBS`, default `true`). Turn it off for
    /// servers that reject the parameter; answers then carry no probability.
    #[serde(default = "default_logprobs")]
    pub logprobs: bool,
}

const fn default_logprobs() -> bool {
    true
}

impl ProviderConfig {
    /// An OpenAI-compatible provider serving `model` at `base_url`, with no
    /// key, no calibration and log-probabilities requested.
    pub fn new(base_url: Url, model: impl Into<String>) -> Self {
        Self {
            base_url,
            api_key: None,
            model: model.into(),
            protocol: Protocol::default(),
            calibration: Calibration::default(),
            external_content: ExternalContent::default(),
            logprobs: default_logprobs(),
        }
    }

    /// The same provider, allowed (or not) to receive mail content when it
    /// is off the instance's network.
    #[must_use]
    pub fn with_external_content(mut self, external_content: ExternalContent) -> Self {
        self.external_content = external_content;
        self
    }

    /// The same provider with a bearer key.
    #[must_use]
    pub fn with_api_key(mut self, key: impl Into<SecretString>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// The same provider speaking `protocol`.
    #[must_use]
    pub fn with_protocol(mut self, protocol: Protocol) -> Self {
        self.protocol = protocol;
        self
    }

    /// The same provider with these `decide` calibration temperatures.
    #[must_use]
    pub fn with_calibration(mut self, calibration: Calibration) -> Self {
        self.calibration = calibration;
        self
    }
}

/// Whether a role whose endpoint is off the instance's network may receive
/// mail content (`CAMPBOOKS_AI__<ROLE>__EXTERNAL_CONTENT`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExternalContent {
    /// No content leaves the instance's network (the default).
    #[default]
    Deny,
    /// The operator opted in: content may be sent to this provider.
    Allow,
}

/// `allow` or `deny`, any case; blank (a key left empty in `.env`) is the
/// default, `deny`.
fn external_content<'de, D: Deserializer<'de>>(d: D) -> Result<ExternalContent, D::Error> {
    let raw = String::deserialize(d)?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "deny" => Ok(ExternalContent::Deny),
        "allow" => Ok(ExternalContent::Allow),
        other => Err(serde::de::Error::custom(format!(
            "EXTERNAL_CONTENT must be `allow` or `deny`, not `{other}`"
        ))),
    }
}

/// Where a role's endpoint is, as far as mail content is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Location {
    /// On the instance's own network: a loopback or private address, a
    /// single-label host name (a compose service such as `laya` or
    /// `ollama`), `localhost`, or a host listed in
    /// `CAMPBOOKS_AI__INTERNAL_HOSTS`.
    Instance,
    /// Anywhere else. Gets no mail content unless the role opts in.
    External,
}

/// Provider per role. Any role may be left unconfigured; calling it then
/// fails with [`Error::RoleNotConfigured`].
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AiConfig {
    /// The configured roles.
    #[serde(flatten)]
    pub roles: HashMap<Role, ProviderConfig>,
    /// Further host names on the instance's own network
    /// (`CAMPBOOKS_AI__INTERNAL_HOSTS`, comma-separated): an exact name, or
    /// `.example.internal` for every name under that domain. Loopback and
    /// private addresses, `localhost` and single-label names need no entry.
    #[serde(default, deserialize_with = "host_list")]
    pub internal_hosts: Vec<String>,
    /// Model calls a workspace may make per UTC day
    /// (`CAMPBOOKS_AI__DAILY_CALLS_PER_WORKSPACE`); read it through
    /// [`daily_call_cap`](Self::daily_call_cap), which applies the default.
    #[serde(default, deserialize_with = "crate::usage::daily_calls")]
    pub daily_calls_per_workspace: Option<u32>,
}

/// A comma-separated string or a list, trimmed, lower-cased, blanks dropped.
fn host_list<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        One(String),
        Many(Vec<String>),
    }
    let raw = match Raw::deserialize(d)? {
        Raw::One(text) => text.split(',').map(str::to_owned).collect(),
        Raw::Many(list) => list,
    };
    Ok(raw
        .iter()
        .map(|host| host.trim().to_ascii_lowercase())
        .filter(|host| !host.is_empty())
        .collect())
}

impl AiConfig {
    /// Load from `CAMPBOOKS_AI__<ROLE>__BASE_URL`, `…__MODEL` and optional
    /// `…__API_KEY` and `…__EXTERNAL_CONTENT`, for `<ROLE>` in `DECIDE`,
    /// `EMBED`, `COMPLETE`, `VISION`, and `CAMPBOOKS_AI__INTERNAL_HOSTS`.
    pub fn from_env() -> Result<Self, Error> {
        Self::from_figment(&env_source())
    }

    /// Load the `ai` section of any figment source. A role with neither a
    /// base URL nor a model is not configured, whatever else is set for it
    /// (an `EXTERNAL_CONTENT` opt-in kept in a compose file while the
    /// role's other settings are commented out).
    pub fn from_figment(figment: &Figment) -> Result<Self, Error> {
        if !figment.contains("ai") {
            return Ok(Self::default());
        }
        let mut section: Dict = figment
            .extract_inner("ai")
            .map_err(|e| Error::Config(e.to_string()))?;
        section.retain(|key, value| {
            let is_role = Role::ALL.iter().any(|role| role.as_str() == key);
            !is_role
                || matches!(value, Value::Dict(_, role)
                    if role.contains_key("base_url") || role.contains_key("model"))
        });
        let mut config: Self = Figment::from(Serialized::defaults(section))
            .extract()
            .map_err(|e| Error::Config(e.to_string()))?;
        // `CAMPBOOKS_AI__<ROLE>__API_KEY=` (left empty in .env) means no key.
        for provider in config.roles.values_mut() {
            if provider
                .api_key
                .as_ref()
                .is_some_and(|key| key.expose_secret().trim().is_empty())
            {
                provider.api_key = None;
            }
        }
        config.validate()?;
        Ok(config)
    }

    /// Reject settings that only make sense for `decide` on other roles.
    pub fn validate(&self) -> Result<(), Error> {
        for (role, provider) in &self.roles {
            if *role == Role::Decide {
                continue;
            }
            let upper = role.as_str().to_uppercase();
            if provider.protocol != Protocol::OpenAi {
                return Err(Error::Config(format!(
                    "CAMPBOOKS_AI__{upper}__PROTOCOL={} is only valid for the decide role; \
                     {role} speaks openai",
                    provider.protocol.as_str()
                )));
            }
            if !provider.calibration.is_empty() {
                return Err(Error::Config(format!(
                    "CAMPBOOKS_AI__{upper}__CALIBRATION__* is only valid for the decide role"
                )));
            }
        }
        Ok(())
    }

    /// The provider for `role`.
    pub fn provider(&self, role: Role) -> Result<&ProviderConfig, Error> {
        self.roles.get(&role).ok_or(Error::RoleNotConfigured(role))
    }

    /// Where `provider`'s endpoint is.
    pub fn location(&self, provider: &ProviderConfig) -> Location {
        if on_instance_network(&provider.base_url, &self.internal_hosts) {
            Location::Instance
        } else {
            Location::External
        }
    }

    /// Whether `provider` may receive mail content: it is on the instance's
    /// network, or its role opted in.
    pub fn content_allowed(&self, provider: &ProviderConfig) -> bool {
        self.location(provider) == Location::Instance
            || provider.external_content == ExternalContent::Allow
    }

    /// The provider for `role`, if it may receive mail content: what every
    /// call carrying content goes through. Fails with
    /// [`Error::RoleNotConfigured`] or [`Error::ExternalContentRefused`].
    pub fn provider_for_content(&self, role: Role) -> Result<&ProviderConfig, Error> {
        let provider = self.provider(role)?;
        if self.content_allowed(provider) {
            Ok(provider)
        } else {
            Err(Error::ExternalContentRefused {
                role,
                host: provider.base_url.host_str().unwrap_or_default().to_owned(),
            })
        }
    }
}

/// Whether `url`'s host is on the instance's own network. Decided from the
/// URL and the configured list only, never from DNS (which can change
/// between this check and the request):
///
/// - an IP address: loopback, RFC 1918 private (`10/8`, `172.16/12`,
///   `192.168/16`) or IPv6 unique-local (`fc00::/7`); not link-local, not
///   carrier-grade NAT, not public;
/// - a name: `localhost` or `*.localhost`, a single label (`laya`,
///   `ollama`: how compose services reach each other), or listed in
///   `internal_hosts` (exactly, or under a `.domain` entry).
pub(crate) fn on_instance_network(url: &Url, internal_hosts: &[String]) -> bool {
    match url.host() {
        Some(Host::Ipv4(ip)) => private_v4(ip),
        Some(Host::Ipv6(ip)) => private_v6(ip),
        Some(Host::Domain(name)) => {
            let name = name.trim_end_matches('.').to_ascii_lowercase();
            if let Ok(ip) = name.parse::<IpAddr>() {
                return match ip {
                    IpAddr::V4(ip) => private_v4(ip),
                    IpAddr::V6(ip) => private_v6(ip),
                };
            }
            name == "localhost"
                || name.ends_with(".localhost")
                || !name.contains('.')
                || internal_hosts.iter().any(|entry| {
                    if entry.starts_with('.') {
                        name.ends_with(entry.as_str())
                    } else {
                        name == *entry
                    }
                })
        }
        None => false,
    }
}

const fn private_v4(ip: Ipv4Addr) -> bool {
    ip.is_loopback() || ip.is_private()
}

const fn private_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return private_v4(v4);
    }
    ip.is_loopback() || ip.is_unique_local()
}

#[cfg(test)]
mod tests {
    use figment::{Figment, providers::Serialized};

    use super::*;
    use crate::decide::QuestionType;

    #[test]
    fn loads_configured_roles_only() {
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "ai": {
                "decide": {"base_url": "http://ollama:11434/v1", "model": "qwen3:8b"},
                "embed": {
                    "base_url": "http://tei/v1",
                    "model": "BAAI/bge-m3",
                    "api_key": "sk-test"
                }
            }
        })));
        let config = AiConfig::from_figment(&figment).unwrap();

        assert_eq!(config.provider(Role::Decide).unwrap().model, "qwen3:8b");
        let embed = config.provider(Role::Embed).unwrap();
        assert_eq!(embed.api_key.as_ref().unwrap().expose_secret(), "sk-test");
        assert!(matches!(
            config.provider(Role::Vision),
            Err(Error::RoleNotConfigured(Role::Vision))
        ));
    }

    #[test]
    fn missing_section_is_empty() {
        let config = AiConfig::from_figment(&Figment::new()).unwrap();
        assert!(config.roles.is_empty());
    }

    #[test]
    fn rejects_partial_role() {
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "ai": {"complete": {"base_url": "http://x.example.com/v1"}}
        })));
        let err = AiConfig::from_figment(&figment).unwrap_err();
        assert!(err.to_string().contains("model"), "{err}");
    }

    #[test]
    fn existing_settings_keep_working() {
        // A decide role set up before the typed protocol existed is an
        // instruct model over the OpenAI-compatible API.
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "ai": {"decide": {"base_url": "http://ollama:11434/v1", "model": "qwen3:8b"}}
        })));
        let decide = AiConfig::from_figment(&figment).unwrap().roles[&Role::Decide].clone();
        assert_eq!(decide.protocol, Protocol::OpenAi);
        assert!(decide.calibration.is_empty());
        assert!(decide.logprobs);
    }

    #[test]
    fn decide_over_laya_with_calibration() {
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "ai": {"decide": {
                "base_url": "http://laya:8000/v1",
                "model": "convaiinnovations/laya-multilingual",
                "protocol": "laya",
                "api_key": "k",
                "calibration": {"choice": 1.5, "choice_2": 1.9, "noul": 2}
            }}
        })));
        let config = AiConfig::from_figment(&figment).unwrap();
        let decide = config.provider(Role::Decide).unwrap();
        assert_eq!(decide.protocol, Protocol::Laya);
        let c = &decide.calibration;
        assert!((c.temperature(QuestionType::Choice, 2) - 1.9).abs() < f64::EPSILON);
        assert!((c.temperature(QuestionType::Choice, 4) - 1.5).abs() < f64::EPSILON);
        assert!((c.temperature(QuestionType::Noul, 2) - 2.0).abs() < f64::EPSILON);
        assert!((c.temperature(QuestionType::Score, 3) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn rejects_unknown_protocol_and_calibration_keys() {
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "ai": {"decide": {"base_url": "http://x/v1", "model": "m", "protocol": "grpc"}}
        })));
        assert!(AiConfig::from_figment(&figment).is_err());

        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "ai": {"decide": {"base_url": "http://x/v1", "model": "m",
                              "calibration": {"choices": 1.2}}}
        })));
        let err = AiConfig::from_figment(&figment).unwrap_err().to_string();
        assert!(err.contains("unknown calibration key `choices`"), "{err}");
    }

    #[test]
    fn decide_only_settings_are_refused_on_other_roles() {
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "ai": {"embed": {"base_url": "http://x/v1", "model": "m", "protocol": "laya"}}
        })));
        let err = AiConfig::from_figment(&figment).unwrap_err().to_string();
        assert!(err.contains("CAMPBOOKS_AI__EMBED__PROTOCOL=laya"), "{err}");

        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "ai": {"complete": {"base_url": "http://x/v1", "model": "m",
                                "calibration": {"noul": 2.0}}}
        })));
        let err = AiConfig::from_figment(&figment).unwrap_err().to_string();
        assert!(err.contains("CAMPBOOKS_AI__COMPLETE__CALIBRATION"), "{err}");
    }

    #[test]
    fn empty_api_key_is_no_key() {
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "ai": {"decide": {"base_url": "http://laya:8000/v1", "model": "m", "api_key": ""}}
        })));
        let config = AiConfig::from_figment(&figment).unwrap();
        assert!(config.provider(Role::Decide).unwrap().api_key.is_none());
    }

    fn url(s: &str) -> Url {
        s.parse().unwrap()
    }

    #[test]
    fn the_instance_network_is_loopback_private_and_single_label() {
        let listed = vec![
            "models.lan.example.com".to_owned(),
            ".corp.example".to_owned(),
        ];
        for inside in [
            "http://laya:8000/v1",
            "http://ollama:11434/v1",
            "http://localhost:8080/v1",
            "http://tei.localhost/v1",
            "http://127.0.0.1:9000/v1",
            "http://10.1.2.3/v1",
            "http://172.20.0.5/v1",
            "http://192.168.1.10/v1",
            "http://[::1]:8000/v1",
            "http://[fd12::1]/v1",
            "http://models.lan.example.com/v1",
            "http://gpu.corp.example/v1",
        ] {
            assert!(on_instance_network(&url(inside), &listed), "{inside}");
        }
        for outside in [
            "https://api.mistral.ai/v1",
            "https://api.example.com/v1",
            "http://8.8.8.8/v1",
            "http://169.254.169.254/v1",
            "http://100.64.0.1/v1",
            "http://172.32.0.1/v1",
            "http://[2001:db8::1]/v1",
            "http://lan.example.com/v1",
            "http://corp.example.evil.com/v1",
        ] {
            assert!(!on_instance_network(&url(outside), &listed), "{outside}");
        }
    }

    #[test]
    fn an_external_role_needs_an_explicit_opt_in() {
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "ai": {
                "decide": {"base_url": "http://laya:8000/v1", "model": "laya"},
                "complete": {"base_url": "https://api.example.com/v1", "model": "m"},
                "embed": {"base_url": "https://embed.example.com/v1", "model": "e",
                          "external_content": "allow"},
                "vision": {"base_url": "https://vision.example.com/v1", "model": "v",
                           "external_content": ""}
            }
        })));
        let config = AiConfig::from_figment(&figment).unwrap();
        assert!(
            config.provider_for_content(Role::Decide).is_ok(),
            "on the box"
        );
        assert!(config.provider_for_content(Role::Embed).is_ok(), "opted in");
        for role in [Role::Complete, Role::Vision] {
            assert!(matches!(
                config.provider_for_content(role),
                Err(Error::ExternalContentRefused { role: r, .. }) if r == role
            ));
            assert!(
                config.provider(role).is_ok(),
                "still configured, for the probe"
            );
        }
        let embed = config.provider(Role::Embed).unwrap();
        assert_eq!(config.location(embed), Location::External);
    }

    #[test]
    fn internal_hosts_are_explicit_configuration() {
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "ai": {
                "internal_hosts": " GPU.Example.Internal , ,.lan.example ",
                "complete": {"base_url": "http://gpu.example.internal/v1", "model": "m"}
            }
        })));
        let config = AiConfig::from_figment(&figment).unwrap();
        assert_eq!(
            config.internal_hosts,
            ["gpu.example.internal", ".lan.example"]
        );
        assert!(config.provider_for_content(Role::Complete).is_ok());
    }

    #[test]
    fn a_lone_opt_in_configures_nothing() {
        // The compose file keeps EXTERNAL_CONTENT while the role's other
        // settings in .env are commented out.
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "ai": {"complete": {"external_content": "allow"}}
        })));
        let config = AiConfig::from_figment(&figment).unwrap();
        assert!(matches!(
            config.provider(Role::Complete),
            Err(Error::RoleNotConfigured(Role::Complete))
        ));
    }

    #[test]
    fn an_unknown_opt_in_value_is_refused() {
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "ai": {"complete": {"base_url": "https://x.example.com/v1", "model": "m",
                                "external_content": "yes"}}
        })));
        let err = AiConfig::from_figment(&figment).unwrap_err().to_string();
        assert!(err.contains("EXTERNAL_CONTENT must be"), "{err}");
    }

    #[test]
    fn debug_hides_api_key() {
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "ai": {"decide": {"base_url": "http://x/v1", "model": "m", "api_key": "sk-secret"}}
        })));
        let config = AiConfig::from_figment(&figment).unwrap();
        assert!(!format!("{config:?}").contains("sk-secret"));
    }
}
