//! Known **open-weight** models, as defaults and suggestions.
//!
//! This list only seeds setup screens and docs. Users and instance admins may
//! configure any model their provider serves; nothing in the gateway checks a
//! model against this catalog. Only open-weight models belong here (see the
//! AI rules in `AGENTS.md` and ADR 0005).

use crate::Role;

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
}

impl ModelInfo {
    /// Whether this model suits `role`.
    pub fn suits(&self, role: Role) -> bool {
        self.roles.contains(&role)
    }
}

/// The catalog.
pub const MODELS: &[ModelInfo] = &[
    // --- decide / complete -------------------------------------------------
    ModelInfo {
        id: "meta-llama/Llama-3.1-8B-Instruct",
        family: "Llama 3.1",
        license: License::Llama31Community,
        roles: &[Role::Decide, Role::Complete],
        embedding_dims: None,
        context_tokens: 131_072,
        ollama_tag: Some("llama3.1:8b"),
    },
    ModelInfo {
        id: "Qwen/Qwen3-8B",
        family: "Qwen3",
        license: License::Apache2,
        roles: &[Role::Decide, Role::Complete],
        embedding_dims: None,
        context_tokens: 32_768,
        ollama_tag: Some("qwen3:8b"),
    },
    ModelInfo {
        id: "mistralai/Mistral-Small-3.2-24B-Instruct-2506",
        family: "Mistral Small 3.2",
        license: License::Apache2,
        roles: &[Role::Decide, Role::Complete, Role::Vision],
        embedding_dims: None,
        context_tokens: 131_072,
        ollama_tag: Some("mistral-small3.2:24b"),
    },
    ModelInfo {
        id: "google/gemma-3-12b-it",
        family: "Gemma 3",
        license: License::Gemma,
        roles: &[Role::Decide, Role::Complete, Role::Vision],
        embedding_dims: None,
        context_tokens: 131_072,
        ollama_tag: Some("gemma3:12b"),
    },
    ModelInfo {
        id: "openai/gpt-oss-20b",
        family: "gpt-oss",
        license: License::Apache2,
        roles: &[Role::Decide, Role::Complete],
        embedding_dims: None,
        context_tokens: 131_072,
        ollama_tag: Some("gpt-oss:20b"),
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
    },
    ModelInfo {
        id: "intfloat/multilingual-e5-large",
        family: "E5",
        license: License::Mit,
        roles: &[Role::Embed],
        embedding_dims: Some(1024),
        context_tokens: 512,
        ollama_tag: None,
    },
    ModelInfo {
        id: "nomic-ai/nomic-embed-text-v1.5",
        family: "Nomic Embed",
        license: License::Apache2,
        roles: &[Role::Embed],
        embedding_dims: Some(768),
        context_tokens: 8192,
        ollama_tag: Some("nomic-embed-text"),
    },
];

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
    fn find_by_id() {
        assert_eq!(
            find("BAAI/bge-m3").and_then(|m| m.embedding_dims),
            Some(1024)
        );
        assert!(find("not/a-model").is_none());
    }
}
