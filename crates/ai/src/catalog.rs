//! Known **open-weight** models, as defaults and suggestions.
//!
//! This list only seeds setup screens and docs. Users and instance admins may
//! configure any model their provider serves; nothing in the gateway checks a
//! model against this catalog. Only open-weight models belong here (see the
//! AI rules in `AGENTS.md` and ADR 0005).
//!
//! `decide` defaults to a Laya decision model served by `laya-serve`
//! (ADR 0009). The instruct models below can still answer `decide` through the
//! `openai` protocol; they are just not suggested for it.

use crate::{Protocol, Role};

/// Licence a model's weights ship under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum License {
    /// Apache License 2.0.
    Apache2,
    /// MIT License.
    Mit,
    /// Llama 3.1 Community License (open weights, use restrictions apply).
    Llama31Community,
    /// Gemma Terms of Use (open weights, use restrictions apply).
    Gemma,
}

impl License {
    /// SPDX identifier where one exists, otherwise a descriptive name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Apache2 => "Apache-2.0",
            Self::Mit => "MIT",
            Self::Llama31Community => "Llama 3.1 Community License",
            Self::Gemma => "Gemma Terms of Use",
        }
    }
}

/// One catalog entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelInfo {
    /// Canonical identifier (the Hugging Face repository).
    pub id: &'static str,
    /// Model family, for grouping in UIs.
    pub family: &'static str,
    /// Weights licence.
    pub license: License,
    /// Roles the model is a sensible default for.
    pub roles: &'static [Role],
    /// Output dimensions, for embedding models.
    pub embedding_dims: Option<u32>,
    /// Context window in tokens (native, without rope-scaling tricks).
    pub context_tokens: u32,
    /// Tag in the Ollama library, when one exists, for one-command local setup.
    pub ollama_tag: Option<&'static str>,
    /// The protocol a server for this model speaks.
    pub protocol: Protocol,
}

impl ModelInfo {
    /// Whether this model suits `role`.
    pub fn suits(&self, role: Role) -> bool {
        self.roles.contains(&role)
    }
}

/// The catalog.
pub const MODELS: &[ModelInfo] = &[
    // --- decide (typed decisions, laya-serve) ------------------------------
    // Context is the checkpoint's shipped limit; the multilingual encoder
    // reads up to 8192 tokens when a request raises it.
    ModelInfo {
        id: "convaiinnovations/laya-multilingual",
        family: "Laya",
        license: License::Apache2,
        roles: &[Role::Decide],
        embedding_dims: None,
        context_tokens: 1024,
        ollama_tag: None,
        protocol: Protocol::Laya,
    },
    ModelInfo {
        id: "convaiinnovations/laya",
        family: "Laya",
        license: License::Apache2,
        roles: &[Role::Decide],
        embedding_dims: None,
        context_tokens: 512,
        ollama_tag: None,
        protocol: Protocol::Laya,
    },
    ModelInfo {
        id: "convaiinnovations/laya-typed-decisions",
        family: "Laya",
        license: License::Apache2,
        roles: &[Role::Decide],
        embedding_dims: None,
        context_tokens: 1024,
        ollama_tag: None,
        protocol: Protocol::Laya,
    },
    // --- complete ----------------------------------------------------------
    ModelInfo {
        id: "meta-llama/Llama-3.1-8B-Instruct",
        family: "Llama 3.1",
        license: License::Llama31Community,
        roles: &[Role::Complete],
        embedding_dims: None,
        context_tokens: 131_072,
        ollama_tag: Some("llama3.1:8b"),
        protocol: Protocol::OpenAi,
    },
    ModelInfo {
        id: "Qwen/Qwen3-8B",
        family: "Qwen3",
        license: License::Apache2,
        roles: &[Role::Complete],
        embedding_dims: None,
        context_tokens: 32_768,
        ollama_tag: Some("qwen3:8b"),
        protocol: Protocol::OpenAi,
    },
    ModelInfo {
        id: "mistralai/Mistral-Small-3.2-24B-Instruct-2506",
        family: "Mistral Small 3.2",
        license: License::Apache2,
        roles: &[Role::Complete, Role::Vision],
        embedding_dims: None,
        context_tokens: 131_072,
        ollama_tag: Some("mistral-small3.2:24b"),
        protocol: Protocol::OpenAi,
    },
    ModelInfo {
        id: "google/gemma-3-12b-it",
        family: "Gemma 3",
        license: License::Gemma,
        roles: &[Role::Complete, Role::Vision],
        embedding_dims: None,
        context_tokens: 131_072,
        ollama_tag: Some("gemma3:12b"),
        protocol: Protocol::OpenAi,
    },
    ModelInfo {
        id: "openai/gpt-oss-20b",
        family: "gpt-oss",
        license: License::Apache2,
        roles: &[Role::Complete],
        embedding_dims: None,
        context_tokens: 131_072,
        ollama_tag: Some("gpt-oss:20b"),
        protocol: Protocol::OpenAi,
    },
    // --- vision ------------------------------------------------------------
    ModelInfo {
        id: "Qwen/Qwen2.5-VL-7B-Instruct",
        family: "Qwen2.5-VL",
        license: License::Apache2,
        roles: &[Role::Vision],
        embedding_dims: None,
        context_tokens: 32_768,
        ollama_tag: Some("qwen2.5vl:7b"),
        protocol: Protocol::OpenAi,
    },
    // --- embed -------------------------------------------------------------
    ModelInfo {
        id: "BAAI/bge-m3",
        family: "BGE",
        license: License::Mit,
        roles: &[Role::Embed],
        embedding_dims: Some(1024),
        context_tokens: 8192,
        ollama_tag: Some("bge-m3"),
        protocol: Protocol::OpenAi,
    },
    ModelInfo {
        id: "intfloat/multilingual-e5-large",
        family: "E5",
        license: License::Mit,
        roles: &[Role::Embed],
        embedding_dims: Some(1024),
        context_tokens: 512,
        ollama_tag: None,
        protocol: Protocol::OpenAi,
    },
    ModelInfo {
        id: "nomic-ai/nomic-embed-text-v1.5",
        family: "Nomic Embed",
        license: License::Apache2,
        roles: &[Role::Embed],
        embedding_dims: Some(768),
        context_tokens: 8192,
        ollama_tag: Some("nomic-embed-text"),
        protocol: Protocol::OpenAi,
    },
];

/// Default model per role, where one has been decided (`complete` and
/// `vision` are still open; see `openspec/decisions-needed.md`).
const DEFAULTS: &[(Role, &str)] = &[
    (Role::Decide, "convaiinnovations/laya-multilingual"),
    (Role::Embed, "BAAI/bge-m3"),
];

/// The default model for `role`, if one has been decided.
pub fn default_for(role: Role) -> Option<&'static ModelInfo> {
    DEFAULTS
        .iter()
        .find(|(r, _)| *r == role)
        .and_then(|(_, id)| find(id))
}

/// Catalog models that suit `role`.
pub fn for_role(role: Role) -> impl Iterator<Item = &'static ModelInfo> {
    MODELS.iter().filter(move |m| m.suits(role))
}

/// Look a model up by its canonical id.
pub fn find(id: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().find(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn ids_are_unique() {
        let ids: HashSet<_> = MODELS.iter().map(|m| m.id).collect();
        assert_eq!(ids.len(), MODELS.len());
    }

    #[test]
    fn every_role_has_a_suggestion() {
        for role in Role::ALL {
            assert!(
                for_role(role).next().is_some(),
                "no catalog model for {role}"
            );
        }
    }

    #[test]
    fn embedding_models_and_only_they_declare_dims() {
        for model in MODELS {
            assert_eq!(
                model.suits(Role::Embed),
                model.embedding_dims.is_some(),
                "{}",
                model.id
            );
        }
    }

    #[test]
    fn decide_defaults_to_laya_multilingual() {
        let model = default_for(Role::Decide).unwrap();
        assert_eq!(model.id, "convaiinnovations/laya-multilingual");
        assert_eq!(model.protocol, Protocol::Laya);
        assert_eq!(model.license.name(), "Apache-2.0");
        // Every decide suggestion is a decision model; instruct models still
        // work for decide over `openai`, but are not suggested for it.
        assert!(for_role(Role::Decide).all(|m| m.protocol == Protocol::Laya));
        assert_eq!(for_role(Role::Decide).count(), 3);
    }

    #[test]
    fn defaults_exist_and_suit_their_role() {
        for (role, id) in DEFAULTS {
            let model = find(id).unwrap();
            assert!(model.suits(*role), "{id} for {role}");
            assert!(model.protocol == Protocol::Laya || *role != Role::Decide);
        }
        assert!(default_for(Role::Complete).is_none(), "not decided yet");
    }

    #[test]
    fn laya_only_serves_decide() {
        for model in MODELS.iter().filter(|m| m.protocol == Protocol::Laya) {
            assert_eq!(model.roles, &[Role::Decide], "{}", model.id);
        }
    }

    #[test]
    fn find_by_id() {
        assert_eq!(
            find("BAAI/bge-m3").and_then(|m| m.embedding_dims),
            Some(1024)
        );
        assert!(find("not/a-model").is_none());
    }
}
