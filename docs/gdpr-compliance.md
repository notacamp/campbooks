# GDPR Compliance — Campbooks

Status of this doc: **working roadmap** (audit 2026-06-21; implementation 2026-06-22).
Tracks the gap register and the phased plan to close it. Engineering items I can build;
items marked **[legal]** need counsel review before relying on them.

> **Status 2026-06-22 — engineering tasks #1–#11 shipped & spec-verified (97-example regression green, i18n parity, 0 new Brakeman warnings):**
> `/jobs` admin-locked · fonts self-hosted (app + website) · terms-consent at signup · 30-day session TTL · password min-8 · full account erasure + OAuth revocation (deletion **and** disconnect, shared-grant-aware) · **Mistral (EU) as the managed AI default** + region disclosure · operational-data retention jobs · governance drafts (`docs/gdpr/`) · "Download my data" export.
> **Remaining:** ⚠️ prod runbook for Mistral (set `MISTRAL_API_KEY` + run `rake ai:repoint_managed_text`); the **[legal]** drafts need counsel; optional hardening in **task #12** (legacy AI fallback → Mistral, documents via pixtral, embeddings) + audit logging + email-body encryption at rest.

Campbooks processes a high volume of **third-party personal data** (everyone who emails a
connected mailbox is a data subject) and runs **AI profiling** over it, so the bar here is
higher than a typical SaaS. The company is the **controller** (per the existing privacy
policy: Not A Camp LDA, Portugal).

---

## Already in good shape

- **EU hosting at rest** — single Hetzner box (Germany); Postgres + Active Storage + OpenSearch self-hosted. No US cloud for primary storage.
- **OAuth tokens & integration secrets encrypted at rest** — `ActiveRecord::Encryption` on every `refresh_token` / `access_token` / `api_key` / `auth_secret` (7 models). Keys required at boot.
- **TLS enforced** — `force_ssl`, HSTS, STARTTLS for SMTP.
- **Per-user / per-workspace access control** — `EmailMessage.accessible_to` / `CalendarEvent.accessible_to` fail closed; all other data scoped through `Current.workspace`.
- **Error monitoring avoids PII** — self-hosted GlitchTip with `send_default_pii = false`.
- **Zoho is EU-resident** (`.zoho.eu` for mail/drive/calendar) — the best-positioned provider.
- **Privacy Policy + Terms already drafted** (website, dated 2026-06-20) — substantive, name the controller, EU hosting, GDPR rights. **Not yet lawyer-reviewed** (flagged in-file).
- **Password hashing** (bcrypt), **login/reset rate-limiting**, **CI security scans** (Brakeman, bundler-audit, importmap audit, Dependabot).

---

## Gap register

### 1. Data-subject rights (Art. 15–22) — biggest engineering gap
- **Account deletion — IN-FLIGHT (uncommitted, ~85% done, built 2026-06-21 by a parallel effort).** `Accounts::Deleter` + `AccountDeletionJob` + danger-zone UI + i18n (4 locales) + **21 passing specs**. Correctly unwinds the `restrict_with_error` cascades in dependency order, nullifies FK landmines (`reviewed_by_id` etc.), purges Active Storage blobs, reassigns sole-owned accounts for multi-member workspaces, and blocks sign-in while a deletion is pending. **Remaining gap:** OAuth revocation (see below). My earlier "no deletion exists" audit predated these files.
- ~~**No user-scoped data export**~~ **RESOLVED 2026-06-22** — Settings → Account → "Download my data" streams `Accounts::DataExporter` as JSON (profile, sessions, AI conversations, signatures, notifications, bug reports + connected-account metadata; honours per-account read access). Art. 15/20. (Synchronous JSON; bulk mailbox/document *content* lives in the user's own connected mailbox and is summarised, not re-dumped.)
- **"Disconnect"** now **revokes the OAuth grant** on user-initiated disconnect (shared-grant-aware, `Accounts::TokenRevoker`) but intentionally **keeps ingested data** — full erasure is via account deletion (#6). An *optional* "also delete ingested data on disconnect" UI is a deferred convenience (not a compliance gap).
- ~~**No OAuth token revocation.**~~ **RESOLVED (deletion path) 2026-06-21.** `revoke_token` now implemented on all three clients — Google (`oauth2.googleapis.com/revoke`) and Zoho (`accounts.zoho.eu/oauth/v2/token/revoke`) hit real endpoints + drop the cached access token; **Microsoft** has no per-refresh-token revoke for our delegated scopes (needs admin `revokeSignInSessions`) so it's a transparent, logged best-effort that returns false. `Accounts::Deleter` now revokes for real. Covered by `spec/services/{google,zoho,microsoft}/oauth_client_spec.rb`.
  - ⚠️ **Shared-grant hazard (blocks naive disconnect-revocation):** a Google/Zoho calendar account shares the *same* refresh token as its sibling mailbox (calendar rides the mail OAuth grant). Revoking on full account *deletion* is safe (the whole workspace is torn down together), but revoking on *disconnect* of a single account would silently break its sibling. Disconnect-revocation must first check no other active account shares the token. **RESOLVED 2026-06-22** — `Accounts::TokenRevoker` does exactly that, wired into both disconnect actions (revokes only when no active sibling shares the grant).

### 2. International transfers (Art. 44–49) — sharpest ongoing legal risk
- ~~**DeepSeek (China) is a selectable text provider**~~ **MITIGATED 2026-06-22** — DeepSeek is **no longer the managed default**; the managed cloud text default is now **Mistral (Paris/EU)** (`Ai::Platform::MANAGED_TEXT_PROVIDER = "mistral"`). DeepSeek stays BYO-selectable, but its region (China) is disclosed in the AI settings picker (`PROVIDER_REGIONS`).
- **Hardcoded Anthropic (US) fallback** — the *managed* default is now Mistral (EU). Residual US exposure = the legacy `call_legacy` path (interactive AI for a workspace with **no** config + `ANTHROPIC_API_KEY`); routing that to Mistral is a tracked follow-up. Background auto-processing is gated by the strict `configured?` and doesn't hit it.
- **Document analysis sends full base64 PDFs** to the AI provider — bank statements, NIFs, IBANs (financial PII, the most sensitive category here).
- **Contact analyzer sends up to 30 historical email bodies** per run (bulk third-party data).
- Other US flows if configured: OpenAI, Gemini, Notion (doc metadata), Google Drive (full files), GitHub (bug-report reporter name/email), Slack/Discord (user-templated content).

### 3. Lawfulness, consent & transparency (Art. 6, 7, 13, 14)
- ~~**No terms/privacy acceptance at registration**~~ **RESOLVED 2026-06-22** — a consent checkbox (links to Privacy + Terms) is required at signup; `terms_accepted_at` is stamped on the user; enforced server-side in `RegistrationsController`. 4 locales.
- ~~**Privacy/Terms not surfaced in the app**~~ **RESOLVED** — policy links shown at registration *and* on Settings → Account (`marketing_url` helper → public site).
- **Sub-processor list is incomplete** — policy lists only Hetzner; reality includes the AI providers (when the app's own key is used), GitHub, etc.
- **Google Fonts loaded from Google CDN.** ~~App~~ **RESOLVED 2026-06-21** — Inter self-hosted (variable woff2 in `app/assets/fonts/`, `@font-face` in the Tailwind input, Google `<link>`s removed; Playwright-confirmed: font served from `/assets/InterVariable-*.woff2`, zero requests to googleapis/gstatic). **Website RESOLVED 2026-06-22** — Inter + JetBrains Mono + Clash Display self-hosted under `website/static/fonts/` via `@font-face` in `custom.css`; Google/Fontshare loads removed from `docusaurus.config.ts`; production build verified to contain **0** googleapis/gstatic/fontshare references.
- **No cookie policy page.** (Only one essential session cookie is set — `httponly`, `same_site: :lax`, Secure in prod — so a consent *banner* is likely not required once fonts are self-hosted, but a cookie *policy* should exist.)

### 4. Security of processing (Art. 32)
- ~~**No audit logging**~~ **PARTIALLY RESOLVED 2026-06-22** — foundational `AuditEvent` log (Art. 5(2)/32(1)(d)) records sign-in/out (centralised in the `Authentication` concern, so password + native + OAuth all covered), data export, password change, account-deletion request, and admin role changes. Best-effort (never breaks the request); `user_id` nullifies on erasure. **Remaining:** logging *every* data read (email/document views) — deferred (the unbounded part).
- ~~**`/jobs` (Mission Control) auth.**~~ **RESOLVED 2026-06-21.** Was wide open (`http_basic_auth_enabled = false`, no `base_controller_class`, no route constraint → plain `ActionController::Base`). Now gated to admins via `MissionControlController` (`base_controller_class`); covered by `spec/requests/mission_control_jobs_spec.rb`. CLAUDE.md note corrected.
- ~~**Password minimum is 6 chars**~~ **RESOLVED 2026-06-21.** Now a model-level `validates :password, length: { minimum: 8 }, allow_nil: true` (single source of truth across registration/settings/reset), threshold + client hints + all 4 locales aligned to 8. Strength/complexity checks still a future nicety.
- **Active Storage on local disk** in production — file blobs unencrypted unless the host disk is encrypted.
- **OpenSearch security plugin disabled in dev** (`DISABLE_SECURITY_PLUGIN=true`); prod compose not in repo — verify prod requires auth and isn't exposed.
- **CSP is entirely commented out**; rate-limiting covers only login + password reset.
- **Live secrets in plaintext `.env`** on the dev machine (incl. AR-encryption keys) — operationally normal, but confirm prod keys differ and rotate if there's any doubt.

### 5. Accountability & governance (Art. 5(2), 30, 33–35) — **[legal]**, I can draft
- **No Records of Processing Activities** (RoPA, Art. 30).
- **No DPIA** (Art. 35) — likely *required* given large-scale processing of communications + AI profiling.
- **No data-breach notification runbook** (Art. 33/34, 72-hour clock).
- **No DPAs/SCCs** with sub-processors (DeepSeek, Anthropic, OpenAI, Google, Microsoft, Notion, GitHub).
- Existing Privacy Policy + Terms need **lawyer review**.

### 6. Data minimisation & retention (Art. 5(1)(c),(e))
- ~~**No retention policy and no cleanup jobs**~~ **PARTIALLY RESOLVED 2026-06-22** — `RetentionSweepJob` (daily) now prunes **operational data**: email-scan/calendar-sync logs + workflow run-history (90d), dismissed feed cards (30d); plus `SessionsPruneJob` (sessions 30d). **User content** (emails, documents, contacts, calendar events, AI chats) is deliberately **not** auto-deleted — content retention would be an explicit per-workspace opt-in (not built; respects the data-safety rule against silent deletion).
- ~~**Sessions never expire**~~ **RESOLVED 2026-06-21.** 30-day sliding inactivity window enforced server-side (the cookie stays permanent), `updated_at` touched at most once/day; expired sessions rejected on resume (`Authentication#find_session_by_cookie`) + swept daily by `SessionsPruneJob`. Bounds retention of `ip_address`/`user_agent`.
- **`SearchChunk.content` re-materialises PII verbatim** (email bodies, document extraction) as a secondary store.

---

## Roadmap (proposed phasing)

**Phase 0 — Quick wins / derisk (S):**
~~verify `/jobs` auth~~ ✅ · ~~self-host fonts (app + website)~~ ✅ (no third-party font CDN anywhere) · ~~terms-acceptance at registration + in-app policy links~~ ✅ (`terms_accepted_at`, consent gated, 4 locales) · ~~session TTL + prune job~~ ✅ (30-day sliding) · ~~bump password minimum~~ ✅ (model-level min-8, all paths + 4 locales).

**Phase 1 — Right to erasure (L):** *(built in-flight + hardened this session)*
~~self-service account deletion~~ ✅ → ~~teardown service~~ ✅ → ~~OAuth token revocation~~ ✅ (deletion **and** disconnect, shared-grant-aware; MS limited) → **remaining (optional, not a gap):** a "delete ingested data on disconnect" convenience UI.

*Cascade audit (2026-06-21): `Accounts::Deleter` confirmed sound — unwinds `restrict_with_error` in order, cleans search indexes (Searchable `dependent: :destroy` via `destroy_all`), purges Active Storage, nullifies FK landmines, handles multi-member reassignment. `ZohoDriveAccount` is a **global/instance-level** integration (no workspace/user FK) so it's correctly untouched by per-user deletion — though that's a data-model inconsistency vs the workspace-scoped `GoogleDriveAccount`. Minor robustness note: `Deleter#delete!` assumes a non-nil workspace.*

**Phase 2 — Right to access/portability (L):** ✅ **DONE 2026-06-22**
~~"Download my data"~~ → `Accounts::DataExporter` streams a user-scoped JSON export from Settings → Account (synchronous, no job/model). *Future option:* async ZIP bundling document file blobs for very large accounts.

**Phase 3 — International transfers (M–L):** *(core shipped 2026-06-22 — Mistral EU default)*
~~default away from DeepSeek~~ ✅ (managed text default → **Mistral / Paris EU**) · ~~surface provider + region in the AI settings UI~~ ✅ (`PROVIDER_REGIONS`, shown in the picker + card) · **remaining:** route the legacy Anthropic fallback → Mistral, move documents (OpenAI/US) + embeddings (OpenAI/Gemini) to EU, complete the public sub-processor list. ⚠️ **Prod runbook:** (1) set `MISTRAL_API_KEY` in the platform env (else managed AI is unavailable); (2) run `rake ai:repoint_managed_text` once — moves *existing* managed workspaces off DeepSeek/China onto Mistral (new setups already default to it). Data-only, idempotent.

**Phase 4 — Retention & minimisation (M):** *(operational enforcement shipped 2026-06-22)*
~~enforcement jobs~~ ✅ — **Retention schedule:** sessions 30d (`SessionsPruneJob`); scan/sync logs + workflow executions 90d, dismissed feed cards 30d (`RetentionSweepJob`); both daily, prod. **Deferred (opt-in):** configurable per-workspace retention that auto-deletes old **emails/documents** (content) — not built, by design (data-safety).

**Phase 5 — Governance docs **[legal]** (M):** *(drafts ✅ 2026-06-22 in [`docs/gdpr/`](gdpr/) — review-ready, **not legal advice**)*
~~sub-processor register~~ ✅ · ~~RoPA~~ ✅ · ~~DPIA outline~~ ✅ · ~~breach runbook~~ ✅ · ~~cookie policy~~ ✅ → **remaining (counsel):** sign/track DPAs, fill the `[TODO]`s, and push the existing policy/terms + the new sub-processor list through review.

**Later / strategic:** ~~audit logging~~ ✅ (foundation; see above) · email-body encryption at rest · CSP · broader rate-limiting.
