# Sub-processor register — Campbooks

> ⚠️ Review-ready draft, not legal advice. Derived from the code audit in
> `../gdpr-compliance.md`. The **public** version of this list should appear in
> the privacy policy (currently only Hetzner is named there — a gap).

A "sub-processor" here is any third party that processes personal data on
Campbooks' behalf. Some entries are **always-on** (every workspace), others are
**conditional** (only when a workspace connects/configures that integration).

## Always-on

| Sub-processor | Purpose | Personal data | Location | Transfer basis |
|---|---|---|---|---|
| **Hetzner Online GmbH** | All hosting — Postgres (`cb_primary`), Active Storage blobs, OpenSearch, self-hosted GlitchTip | Everything stored: accounts, email/document/calendar content, logs (IP/UA) | Germany (EU) | N/A — EU |

*GlitchTip (error monitoring) is self-hosted on the Hetzner box, with `send_default_pii = false`, so it is not a separate external processor.*

## Conditional — AI providers (the sharpest transfer risk)

Personal data sent: email subjects + bodies, contact histories, calendar text, document **file contents** (incl. financial PII — NIF, IBAN, bank statements), and AI-assistant chat. Routing is per-workspace (`AiConfiguration`). **The managed cloud default is now Mistral (Paris/EU)** (2026-06-22); the remaining US exposure is the legacy Anthropic fallback for interactive AI in an unconfigured workspace.

| Sub-processor | Purpose | Location | Transfer basis | Notes |
|---|---|---|---|---|
| **Mistral AI** | **Default** managed text AI | **France (EU)** | N/A — EU | GDPR-preferred default; replaced DeepSeek as the managed text provider. Needs `MISTRAL_API_KEY`. |
| **Anthropic, PBC** | Text/document AI (BYO) + legacy interactive fallback | USA | SCCs / EU-US DPF | `call_legacy` fallback for unconfigured workspaces; routing it to Mistral is a follow-up. |
| **OpenAI, L.L.C.** | Document AI (managed) + text (BYO) + embeddings | USA | SCCs / EU-US DPF | Still the managed **document** provider (Mistral pixtral is a follow-up). Receives full base64 documents. |
| **DeepSeek** | Text AI (BYO only) | **China** | ⚠️ No adequacy decision | **No longer a default** — Mistral replaced it as managed text (2026-06-22). BYO-selectable only; region (China) disclosed in the AI picker. |
| **Google (Gemini)** | Text/document AI | USA | SCCs / EU-US DPF | Workspace-configured. |

## Conditional — mailbox / calendar / storage connectors

For their own mailbox/calendar service these providers are arguably **independent controllers**; they act as our processor when we fetch and store the user's content.

| Sub-processor | Purpose | Location | Transfer basis |
|---|---|---|---|
| **Zoho Corporation** | Mail + Calendar + WorkDrive sync; transactional SMTP | **EU** (`.zoho.eu`) | EU residency |
| **Google LLC** | Gmail / Calendar / Drive sync | USA | SCCs / EU-US DPF |
| **Microsoft Corporation** | Outlook / Graph mail sync | USA / EU regions | SCCs / EU-US DPF |
| **Notion Labs, Inc.** | Document-metadata sync (if connected) | USA | SCCs / EU-US DPF |

## Conditional — outbound integrations (workspace-configured)

| Sub-processor | Purpose | Location | Transfer basis |
|---|---|---|---|
| **GitHub, Inc.** | Bug-report → issue sync (only if `GITHUB_TOKEN` set) — sends reporter name + email | USA | SCCs / EU-US DPF |
| **Slack / Discord** | Workflow notifications (user-templated content) | USA | SCCs / EU-US DPF |
| **Generic HTTP webhooks** | User-defined workflow steps — any destination | User-chosen | Controller (user) responsibility — surface a warning |

## Action items
- [ ] Put the **always-on + likely-used** entries into the public privacy policy (Hetzner alone is insufficient).
- [ ] Sign/obtain DPAs (Art. 28) with each processor actually used. **[TODO: track which DPAs are in place]**
- [ ] Resolve the **Anthropic default-fallback** US transfer and the **DeepSeek (China)** transfer (task #8) — these are the two that need a decision, not just paperwork.
- [ ] Add a UI warning when a user templates content into a Slack/Discord/HTTP workflow step.

Last updated: 2026-06-22.
