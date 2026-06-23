# Records of Processing Activities (RoPA) — Campbooks

> ⚠️ Review-ready draft, not legal advice (Art. 30). Derived from the code audit.

**Controller:** Not A Camp LDA, Portugal (registered address kept in internal records) · inbox@not-a-camp.com
**DPO / contact:** [TODO — a DPO is not mandatory for an org this size unless core activity is large-scale monitoring; given large-scale comms processing + AI profiling, take advice on whether Art. 37 is triggered.]
**Supervisory authority:** CNPD (Portugal).

## Cross-cutting facts
- **Hosting / storage location:** Hetzner, Germany (EU). No primary storage outside the EU.
- **International transfers:** only via the AI providers and US-based connectors — see `sub-processors.md`.
- **Security measures (all activities):** TLS in transit (`force_ssl`, HSTS); `ActiveRecord::Encryption` for OAuth tokens / integration secrets; per-user + per-workspace access control (`EmailMessage.accessible_to`, `CalendarEvent.accessible_to`, workspace scoping); bcrypt passwords (min 8); admin-gated ops dashboard; CI security scanning (Brakeman, bundler-audit). **Gaps tracked in `../gdpr-compliance.md`** (no audit log; some data at rest unencrypted at column level).
- **Retention:** sessions 30 days (sliding); **most other data is currently retained indefinitely — retention enforcement is task #9.** Deletion honoured via `Accounts::Deleter` (account erasure).

## Processing activities

### A1. Account & authentication
- **Purpose:** create and secure user accounts; sign-in.
- **Legal basis:** Art. 6(1)(b) contract.
- **Data subjects:** registered users.
- **Data:** name, email, password hash, locale, role, session IP + user-agent, native-device tokens.
- **Recipients:** Hetzner. (Zoho SMTP for OTP/transactional mail.)
- **Retention:** life of account; sessions 30 days.

### A2. Mailbox ingestion & AI triage
- **Purpose:** sync the user's mailbox and AI-summarise/triage it.
- **Legal basis:** Art. 6(1)(b) contract (for the user); **Art. 6(1)(f) legitimate interests** for the personal data of third-party senders/recipients we necessarily process.
- **Data subjects:** users **and third parties** who email them.
- **Data:** from/to/cc/bcc, subject, full body, attachments, AI summaries/derived fields.
- **Recipients:** mailbox provider (Google/Microsoft/Zoho), AI provider (see sub-processors), Hetzner.
- **Retention:** indefinite [→ task #9].

### A3. Document management & AI extraction
- **Purpose:** store documents and extract structured fields (invoices, statements, contracts).
- **Legal basis:** Art. 6(1)(b); legitimate interests for third-party data inside documents. **Note: financial identifiers (NIF/tax ID, IBAN) are sensitive though not Art. 9 special category.**
- **Data subjects:** users, vendors, clients, named third parties in documents.
- **Data:** original files, extracted fields (names, tax IDs, bank accounts, amounts), AI analysis.
- **Recipients:** AI provider (full file content as base64), optional Notion/Google Drive/Zoho WorkDrive, Hetzner.
- **Retention:** indefinite [→ task #9].

### A4. Calendar sync
- **Purpose:** two-way calendar sync.
- **Legal basis:** Art. 6(1)(b); legitimate interests for attendee data.
- **Data subjects:** users, event attendees.
- **Data:** event title/description/location, attendee names + emails, RSVP.
- **Recipients:** Google/Zoho, Hetzner.

### A5. Contact intelligence
- **Purpose:** build contact/"person" profiles + communication patterns from mail.
- **Legal basis:** Art. 6(1)(f) legitimate interests. **This is profiling — covered in the DPIA.**
- **Data subjects:** the user's correspondents (third parties).
- **Data:** name, org, email aliases, AI-generated context summaries, behavioural patterns.
- **Recipients:** AI provider, Hetzner.

### A6. AI assistant (Scout)
- **Purpose:** conversational assistant over the user's data.
- **Legal basis:** Art. 6(1)(b).
- **Data:** chat content, prompts (which quote email/doc data), suggested/auto actions.
- **Recipients:** AI provider, Hetzner.

### A7. Search indexing
- **Purpose:** full-text + semantic search.
- **Data:** verbatim excerpts of email bodies / document extractions / contact summaries; vector embeddings.
- **Recipients:** embedding provider (OpenAI/Gemini), OpenSearch on Hetzner.
- **Note:** `search_chunks` re-materialises PII; cleaned on parent destroy.

### A8. Workflow automation
- **Purpose:** user-defined automations on email/webhook triggers.
- **Data:** trigger payloads + user-templated content sent to outbound steps.
- **Recipients:** user-chosen (Slack/Discord/HTTP), Hetzner.

### A9. Notifications & transactional email
- **Legal basis:** Art. 6(1)(b).
- **Data:** notification text, recipient email; OTP/invite mails via Zoho SMTP (EU).

### A10. Bug reporting
- **Purpose:** in-app bug reports.
- **Data:** description, **screenshot (may capture PII-laden UI)**, page URL, user-agent; optional sync to GitHub (reporter name + email → US).
- **Legal basis:** Art. 6(1)(f).

### A11. Logging & security
- **Data:** application logs (parameter-filtered), session IP/UA, GlitchTip error events (`send_default_pii = false`).
- **Legal basis:** Art. 6(1)(f) (security).

Last updated: 2026-06-22.
