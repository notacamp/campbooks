//! Which provider and model serves each AI role.

use std::{collections::HashMap, fmt};

use campbooks_kernel::config::env_source;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::Error;

/// What a call is *for*. Code asks for a role, never for a vendor or model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Small, fast structured judgements (classify, extract, route) — usually
    /// with a JSON schema response format.
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

/// An OpenAI-compatible endpoint and the model to use on it.
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    /// API base URL **including the version prefix**, as OpenAI SDKs expect:
    /// `http://ollama:11434/v1`, `http://tei/v1`, `https://api.example.com/v1`.
    /// The gateway appends `chat/completions` / `embeddings`.
    pub base_url: Url,
    /// Bearer token, if the provider needs one. Never logged.
    #[serde(default)]
    pub api_key: Option<SecretString>,
    /// Model identifier as the provider names it (`qwen3:8b`, `BAAI/bge-m3`, …).
    pub model: String,
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
        figment
            .extract_inner("ai")
            .map_err(|e| Error::Config(e.to_string()))
    }

    /// The provider for `role`.
    pub fn provider(&self, role: Role) -> Result<&ProviderConfig, Error> {
        self.roles.get(&role).ok_or(Error::RoleNotConfigured(role))
    }
}

#[cfg(test)]
mod tests {
    use figment::{Figment, providers::Serialized};
    use secrecy::ExposeSecret;

    use super::*;

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
    fn debug_hides_api_key() {
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "ai": {"decide": {"base_url": "http://x/v1", "model": "m", "api_key": "sk-secret"}}
        })));
        let config = AiConfig::from_figment(&figment).unwrap();
        assert!(!format!("{config:?}").contains("sk-secret"));
    }
}
