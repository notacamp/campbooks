# GDPR — blocked & deferred backlog

What's **done** (tasks #1–#11 + the Mistral adapter) is tracked in [`../gdpr-compliance.md`](../gdpr-compliance.md).
This file is the remaining backlog — each item notes **why it isn't done yet** and **what unblocks it**.

Last updated: 2026-06-22.

## 🔒 Blocked on external parties (counsel / ops)

- [ ] **Legal review** of the `docs/gdpr/` drafts (sub-processor register, RoPA, DPIA, breach runbook, cookie policy) **and** the existing website Privacy Policy + Terms. Fill every `[TODO]` placeholder. → *needs a data-protection lawyer.*
- [ ] **Sign / track DPAs (Art. 28)** with each sub-processor actually used (Mistral, OpenAI, Anthropic, Google, Microsoft, Zoho, Notion, GitHub, Slack/Discord). → *contracts.*
- [ ] **Prod runbook for the Mistral default** *(in progress — an agent is setting the key)*: set `MISTRAL_API_KEY` in the platform env, then run `rake ai:repoint_managed_text` to move existing managed workspaces off DeepSeek/China. → *needs prod env access.*

## ⛔ Blocked on infra / verification

- [ ] **Documents → Mistral (pixtral), EU.** Switch `Ai::Platform::MANAGED_DOC_PROVIDER` off OpenAI (US) once verified. → *blocked: need a live `MISTRAL_API_KEY` to confirm the base64 PDF/image payload (`Ai::Adapters::Openai#translate_part`) works with `pixtral-large-latest` before flipping managed docs.*
- [ ] **Embeddings → `mistral-embed`, EU.** Keep semantic search in the EU. → *blocked: dimension mismatch — search vectors are `vector(1536)` (OpenAI `text-embedding-3-small`); `mistral-embed` is 1024-dim. Requires a schema/index migration + **re-embedding all existing content**. Needs a migration + backfill plan.*

## 🧱 Deferred — large / risky (not blocked; schedule when prioritised)

- [ ] **Legacy Anthropic (US) fallback → Mistral.** The `call_legacy` path (interactive AI for a workspace with no AI config + `ANTHROPIC_API_KEY`) still goes to Anthropic/US. *(Being evaluated for centralisation — if there's a single seam it'll move to the done list.)*
- [x] ~~**Audit logging** foundation~~ — DONE 2026-06-22: `AuditEvent` logs sign-in/out, export, password change, deletion, admin role changes. **Remaining:** (a) log *every* data read (email/document views) — the unbounded part; (b) an audit-log **retention** window (currently kept indefinitely); (c) an admin UI to view the audit trail.
- [ ] **Encrypt `email_messages.body` at rest** — highest-density plaintext PII column. Large: perf + search/embedding implications; needs a backfill.
- [ ] **Enable CSP.** `config/initializers/content_security_policy.rb` is fully commented out. Needs careful testing (inline anti-flash theme script in `shared/_theme_head`, tiptap, importmap, fonts).
- [ ] **"Delete ingested data on disconnect" UI** — optional convenience (full erasure already exists via account deletion). Multi-step modal per the setup-modal convention; Playwright-verify.
- [ ] **Async ZIP export** bundling document file blobs for very large accounts (current `Accounts::DataExporter` is synchronous JSON).
- [ ] **Stronger DeepSeek/China warning** in the AI add-provider modal (region is already shown as "DeepSeek · China"; a confirm dialog could be added).
- [ ] **Per-workspace content retention** (opt-in auto-delete of old emails/documents). Deliberately not built — would silently delete user content; needs an explicit setting + the user's window policy.

## 🧪 Pre-existing test failures — NOT from GDPR work (coordinate with the inbox-settings refactor)

The full suite has **16 failures + 1 load error** that predate / are unrelated to the GDPR work (verified: failing files committed/unmodified by GDPR changes; setting `MISTRAL_API_KEY` doesn't fix them; all GDPR specs pass).

- [ ] `spec/controllers/contacts_controller_spec.rb` — load error: references `InboxSettings::ContactsController`, which doesn't exist yet (mid-refactor; `app/controllers/inbox_settings/` has the others but not `contacts`).
- [ ] ~13 invitations/members controller specs 302 — `type: :controller` specs don't get the `SetupStatus#complete? => true` stub that `type: :request` specs get (`spec/support/auth_helper.rb:15`), so the onboarding redirect fires. **Quick win:** add a `config.before(:each, type: :controller)` stub (watch for onboarding-specific controller specs).
- [ ] `spec/models/contact_spec.rb:91` — `DocumentType … Workspace must exist` (creates a `DocumentType` without a workspace).
- [ ] `email_process_job` / `documents_review` — unrelated domain logic.
