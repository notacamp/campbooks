# Decisions needed

Open questions that need a human. One heading per decision, each with the
options and a recommended answer. Remove an entry once it is decided and
recorded (ADR, spec or issue).

## Domain map and system shape (ADR 0008, 0010, 0011)

`docs/domain-map.md` proposes the module list, their language and the
milestone order; ADR 0008 (typed core with platform seams, platform modules
last), ADR 0010 (connectors and change streams) and ADR 0011 (one binary in
three roles, event-log projections, blobs in object storage, row-level
security, client change feed) are **Proposed**.
**Recommended:** accept all three and the map as written; per-module epics
are opened once they are accepted.

## Default `complete` model (drafting, summaries, chat)

The `decide` role is settled (Laya, ADR 0009). The default open-weight model
for `complete` — drafting replies, summarising, the assistant's conversation,
schema-guided extraction — is not. Candidates in the catalog today: Qwen3 8B,
Mistral Small 3.2 24B, Gemma 3 12B, gpt-oss 20B, Llama 3.1 8B. Any of them is
configurable per instance and per workspace regardless of the default.
**Recommended:** let the AI-gateway eval set (drafting quality in en/pt/es/fr,
extraction accuracy, latency and memory on the reference hardware) pick the
default, and decide the hosted-cloud model together with the GPU sizing issue.
