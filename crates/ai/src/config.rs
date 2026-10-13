//! Which provider and model serves each AI role.

use std::{collections::HashMap, fmt};

use campbooks_kernel::config::env_source;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use url::Url;

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
            logprobs: default_logprobs(),
        }
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

/// Provider per role. Any role may be left unconfigured; calling it then
/// fails with [`Error::RoleNotConfigured`].
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(transparent)]
pub struct AiConfig {
    /// The configured roles.
    pub roles: HashMap<Role, ProviderConfig>,
}

impl AiConfig {
    /// Load from `CAMPBOOKS_AI__<ROLE>__BASE_URL`, `…__MODEL` and optional
    /// `…__API_KEY`, for `<ROLE>` in `DECIDE`, `EMBED`, `COMPLETE`, `VISION`.
    pub fn from_env() -> Result<Self, Error> {
        Self::from_figment(&env_source())
    }

    /// Load the `ai` section of any figment source.
    pub fn from_figment(figment: &figment::Figment) -> Result<Self, Error> {
        if !figment.contains("ai") {
            return Ok(Self::default());
        }
        let mut config: Self = figment
            .extract_inner("ai")
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

    #[test]
    fn debug_hides_api_key() {
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "ai": {"decide": {"base_url": "http://x/v1", "model": "m", "api_key": "sk-secret"}}
        })));
        let config = AiConfig::from_figment(&figment).unwrap();
        assert!(!format!("{config:?}").contains("sk-secret"));
    }
}
