# 0005 — Open-weight AI only, addressed by role, fully user-configurable

**Status:** Accepted · 2026-10-12 · `decide` amended by [0009](0009-decide-role-typed-decision-model.md)

## Context
AI is the core of the product (sorting, extraction, drafting, search). We
want independence from any single vendor, the option to self-host every model,
data residency control, and users who can configure everything.

## Decision
- **Only open-weight models** appear in the shipped catalog. They can be
  self-hosted (vLLM, llama.cpp, Ollama, Text Embeddings Inference) or used via
  any hosted endpoint that serves open weights.
- **Code asks for a role, never a vendor or model**:
  - `decide` — small, fast instruct model for classification, routing and
    structured extraction (JSON-schema constrained output);
  - `embed` — embedding model for search and similarity (pgvector);
  - `complete` — larger model for drafting, summarising and chat;
  - `vision` (optional) — documents and images.
- **One protocol:** the OpenAI-compatible HTTP API (`/v1/chat/completions`,
  `/v1/embeddings`), which every serving stack above speaks.
- **Full user configuration:** instance defaults from env
  (`CAMPBOOKS_AI__<ROLE>__BASE_URL|MODEL|API_KEY`); workspace overrides per
  role (base URL, model, key) stored encrypted; a "test connection" per role.
  Every AI feature declares its role and degrades gracefully when the role is
  unset.
- `campbooks-ai` is the only crate that talks to model servers.

## Consequences
- Changing the embedding model invalidates stored vectors: embeddings record
  their model id and dimensions, and a re-embed job is part of the design.
- CPU-only hosts can run `embed` comfortably but are slow for `decide` and
  `complete`; the hosted cloud needs a GPU node or a hosted open-weight
  endpoint. Self-hosters choose.
- Quality is measured, not assumed: an eval set per AI feature picks the
  default model per role.

## Alternatives considered
- Proprietary APIs as defaults — vendor lock-in and data-residency issues.
- One model for everything — wasteful for classification, weak for drafting.
