# Settings, Integrations & Notifications — `/api/app`

**Status:** ❌ not started · **Priority:** P2+ · **Depends on:** `00-auth.md`

Settings is the largest surface by controller count (~30 controllers across two namespaces
plus scattered top-level routes). The SPA needs all of it to be self-serviceable without
a server-rendered page. This file catalogs every sub-area, maps the existing web routes,
and lists the `/api/app` endpoints to build.

## `/api/v1` coverage today

❌ **None.** The public Doorkeeper API has no settings, members, security, integrations,
notifications, or inbox-configuration endpoints. Every endpoint here is net-new.

---

## Account & security

### Web routes

| Web route | Controller#action | Purpose |
|---|---|---|
| `GET/PATCH /settings/account` | `Settings::AccountController#show|update` | Name, email, avatar, locale |
| `PATCH /settings/account/language` | `#language` | Switch UI locale |
| `PATCH /settings/account/compose_preference` | `#compose_preference` | Compose panel layout pref |
| `PATCH /settings/account/writing_style` | `#writing_style` | Save writing-style description |
| `POST /settings/account/analyze_writing_style` | `#analyze_writing_style` | AI-infer style from sent mail |
| `GET /settings/account/delete` | `#delete` | Delete-account confirmation page |
| `DELETE /settings/account` | `#destroy` | Schedule account deletion |
| `POST /settings/account/export` | `#export` | Enqueue data export |
| `GET /settings/account/download_export` | `#download_export` | Download the export archive |
| `GET /settings/security` | `Settings::SecurityController#show` | Security overview (active sessions, 2FA status) |
| `DELETE /settings/security/disable` | `#disable` | Disable all 2FA (password re-auth required) |
| `GET/POST /settings/security/totp/new` | `Settings::Security::TotpController#new|create` | Enroll TOTP |
| `DELETE /settings/security/totp` | `#destroy` | Remove TOTP |
| `GET /settings/security/passkeys/options` | `Settings::Security::PasskeysController#options` | WebAuthn creation options |
| `GET/POST /settings/security/passkeys/new` | `#new|create` | Enroll passkey |
| `DELETE /settings/security/passkeys/:id` | `#destroy` | Remove passkey |
| `GET/POST /settings/security/recovery_codes` | `Settings::Security::RecoveryCodesController#show|create` | View / regenerate recovery codes |
| `POST /settings/security/email_otp` | `Settings::Security::EmailOtpController#create` | Enable email OTP |
| `DELETE /settings/security/email_otp` | `#destroy` | Disable email OTP |
| `POST /settings/security/sign_in_methods` | `Settings::Security::SignInMethodsController#create` | Link OAuth sign-in method |
| `DELETE /settings/security/sign_in_methods/:id` | `#destroy` | Unlink OAuth sign-in method |
| `GET /settings/security/audit_log` | `Settings::Security::AuditLogController#index` | Personal security/audit history |

### `/api/app` endpoints to build

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET /api/app/account` | Current-user profile (name, email, avatar, locale, compose pref, writing style) | `Settings::AccountController` |
| `PATCH /api/app/account` | Update profile fields | `#update` logic |
| `PATCH /api/app/account/language` | Switch locale | `#language` |
| `PATCH /api/app/account/compose_preference` | Compose layout pref | `#compose_preference` |
| `PATCH /api/app/account/writing_style` | Save writing style | `#writing_style` |
| `POST /api/app/account/analyze_writing_style` | AI-infer style | `#analyze_writing_style` |
| `POST /api/app/account/export` | Enqueue data export | `#export` |
| `GET /api/app/account/export` | Download export (signed URL or stream) | `#download_export` |
| `DELETE /api/app/account` | Schedule deletion | `#destroy` |
| `GET /api/app/security` | Security overview (sessions, enrolled 2FA methods) | `#show` |
| `DELETE /api/app/security` | Disable all 2FA | `#disable` |
| `GET /api/app/security/totp/setup` | TOTP provisioning URI + QR data | `Totp#new` logic |
| `POST /api/app/security/totp` | Verify + activate TOTP | `Totp#create` |
| `DELETE /api/app/security/totp` | Remove TOTP | `Totp#destroy` |
| `GET /api/app/security/passkeys/options` | WebAuthn creation options | `Passkeys#options` |
| `POST /api/app/security/passkeys` | Enroll passkey | `Passkeys#create` |
| `DELETE /api/app/security/passkeys/:id` | Remove passkey | `Passkeys#destroy` |
| `GET/POST /api/app/security/recovery_codes` | View / regenerate recovery codes | `RecoveryCodes` |
| `POST /api/app/security/email_otp` | Enable email OTP | `EmailOtp#create` |
| `DELETE /api/app/security/email_otp` | Disable email OTP | `EmailOtp#destroy` |
| `GET /api/app/security/sign_in_methods` | List linked OAuth identities | `SignInMethods` |
| `POST /api/app/security/sign_in_methods` | Initiate link flow | `#create` |
| `DELETE /api/app/security/sign_in_methods/:id` | Unlink identity | `#destroy` |
| `GET /api/app/security/audit_log` | Paginated security events | `AuditLogController` |

---

## Workspace & members

### Web routes

| Web route | Controller#action | Purpose |
|---|---|---|
| `GET/PATCH /settings/general` | `Settings::GeneralController#show|update` | Workspace name, logo, timezone |
| `GET/PATCH /settings/setup_template` | `Settings::SetupTemplateController#show|update` | Onboarding / setup-card template |
| `PATCH /settings/setup_template/update_modules` | `#update_modules` | Toggle which setup-card modules are active |
| `GET/PATCH /settings/plan` | `Settings::PlanController#show` | Billing/plan info |
| `GET /settings/members` | `Settings::MembersController#index` | List workspace members + roles |
| `PATCH /settings/members/:id` | `#update` | Change member role |
| `POST /settings/invitations` | `Settings::InvitationsController#create` | Send invite |
| `DELETE /settings/invitations/:id` | `#destroy` | Revoke invite |
| `POST /settings/invitations/:id/resend` | `#resend` | Re-send invite email |
| `POST /settings/invitations/:id/approve` | `#approve` | Approve a pending (cloud) invitation |
| `GET /settings/system_health` | `Settings::SystemHealthController#show` | Self-hosted health dashboard |

### `/api/app` endpoints to build

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET/PATCH /api/app/settings/workspace` | Workspace name, logo, timezone | `GeneralController` |
| `GET/PATCH /api/app/settings/setup_template` | Setup-card template + module toggles | `SetupTemplateController` |
| `GET /api/app/settings/plan` | Plan/billing info | `PlanController` |
| `GET /api/app/settings/members` | Member list + roles | `MembersController` |
| `PATCH /api/app/settings/members/:id` | Update member role | `#update` |
| `POST /api/app/settings/invitations` | Send invite | `InvitationsController#create` |
| `DELETE /api/app/settings/invitations/:id` | Revoke invite | `#destroy` |
| `POST /api/app/settings/invitations/:id/resend` | Re-send invite | `#resend` |
| `POST /api/app/settings/invitations/:id/approve` | Approve pending invite | `#approve` |
| `GET /api/app/settings/system_health` | Health status JSON | `SystemHealthController` |

---

## Inbox settings

Two namespaces serve inbox configuration: `namespace :settings` (overview pages) and
`namespace :inbox_settings` (the management panels, also loadable stand-alone). Both
feed the same underlying models and services.

### Web routes

| Web route | Controller#action | Purpose |
|---|---|---|
| `GET /settings/inbox/:section` | `Settings::InboxController#show` | Shell page for an inbox-settings panel section |
| `GET /settings/memory` | `Settings::MemoryController#show` | Scout's memory — editable AI habit sentences |
| `POST /settings/memory/teach` | `#teach` | Add a new rule/habit from a sentence |
| `POST /settings/memory/entries/:id/confirm` | `#confirm` | Confirm a learned habit |
| `DELETE /settings/memory/entries/:id` | `#destroy` | Remove a memory entry |
| `GET /inbox_settings/display` | `InboxSettings::DisplayController#show` | Display preferences (density, thread grouping) |
| `GET/PATCH /inbox_settings/filtering` | `InboxSettings::FilteringController#show|update` | Inbox filter strategy (allowlist/blocklist) |
| `POST /inbox_settings/filtering/sender` | `#set_sender` | Block/allow/star a specific sender |
| `GET…DELETE /inbox_settings/tags` | `InboxSettings::TagsController` (CRUD) | Tag management |
| `PATCH /inbox_settings/tags/:id/toggle_hidden` | `#toggle_hidden` | Hide/show a tag in the inbox |
| `GET/POST /inbox_settings/tags/:id/merge` | `#merge|commit_merge` | Merge a tag into another |
| `GET /inbox_settings/label_reviews` | `InboxSettings::LabelReviewsController#index` | Review imported provider labels |
| `PATCH /inbox_settings/label_reviews/bulk_decide` | `#bulk_decide` | Bulk map/keep/ignore labels |
| `GET…DELETE /inbox_settings/tag_groups` | `InboxSettings::TagGroupsController` (CRUD) | Inbox group (stream) management |
| `GET…DELETE /inbox_settings/document_types` | `InboxSettings::DocumentTypesController` (CRUD) | Document-type management |
| `GET…DELETE /inbox_settings/signatures` | `InboxSettings::SignaturesController` (CRUD) | Email signature management |
| `POST /inbox_settings/signatures/:id/set_default` | `#set_default` | Set default signature |
| `GET…DELETE /inbox_settings/rules` | `InboxSettings::RulesController` (CRUD) | Inbox rules |
| `PATCH /inbox_settings/rules/:id/toggle` | `#toggle` | Enable/disable a rule |
| `POST /inbox_settings/rules/:id/run` | `#run` | Run a rule retroactively |
| `GET /inbox_settings/rules/match_count` | `#match_count` | Count emails a rule would match |
| `POST /inbox_settings/rules/:rule_id/runs/:id/undo` | `#undo` | Undo a retroactive run |
| `GET /inbox_settings/accounts` | `InboxSettings::AccountsController#show` | Email accounts overview |
| `POST /inbox_settings/accounts/scan` | `#scan_now` | Trigger a manual inbox scan |

### `/api/app` endpoints to build

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET/PATCH /api/app/inbox_settings/display` | Display preferences | `DisplayController` |
| `GET/PATCH /api/app/inbox_settings/filtering` | Filter strategy | `FilteringController` |
| `POST /api/app/inbox_settings/filtering/sender` | Block/allow/star sender | `#set_sender` |
| `GET/POST/PATCH/DELETE /api/app/inbox_settings/tags` | Tag CRUD + merge + toggle_hidden | `TagsController` |
| `GET/PATCH /api/app/inbox_settings/label_reviews` | Label-import review | `LabelReviewsController` |
| `GET/POST/PATCH/DELETE /api/app/inbox_settings/tag_groups` | Inbox group CRUD | `TagGroupsController` |
| `GET/POST/PATCH/DELETE /api/app/inbox_settings/document_types` | Document-type CRUD | `DocumentTypesController` |
| `GET/POST/PATCH/DELETE /api/app/inbox_settings/signatures` | Signature CRUD + set_default | `SignaturesController` |
| `GET/POST/PATCH/DELETE /api/app/inbox_settings/rules` | Rule CRUD + toggle/run/undo/match_count | `RulesController` |
| `GET /api/app/inbox_settings/accounts` | Email accounts overview | `AccountsController` |
| `POST /api/app/inbox_settings/accounts/scan` | Manual inbox scan | `#scan_now` |
| `GET /api/app/settings/memory` | Scout memory entries (serialized) | `MemoryController` |
| `POST /api/app/settings/memory/teach` | Add memory entry from sentence | `#teach` |
| `POST /api/app/settings/memory/entries/:id/confirm` | Confirm learned habit | `#confirm` |
| `DELETE /api/app/settings/memory/entries/:id` | Remove memory entry | `#destroy` |

---

## Integrations

### Web routes

| Web route | Controller#action | Purpose |
|---|---|---|
| `GET /settings/integrations` | `Settings::Integrations::IndexController#show` | Integrations overview page |
| `GET/PATCH /settings/integrations/notion` | `Settings::Integrations::NotionController#show|update` | Notion integration settings |
| `DELETE /settings/integrations/notion/integrations/:id` | `#destroy` | Disconnect a Notion workspace |
| `GET/DELETE /settings/integrations/google_drive` | `Settings::Integrations::GoogleDriveController#show|destroy` | Google Drive connection |
| `POST /settings/integrations/google_drive/retry_failed` | `#retry_failed` | Retry failed Drive uploads |
| `GET /settings/integrations/google_drive/configs/:document_type_id/edit` | `Settings::Integrations::GoogleDriveConfigsController#edit` | Edit per-doc-type Drive folder config |
| `GET /settings/integrations/google_drive/configs/:document_type_id/folders` | `#browse_folders` | Browse Drive folders (picker) |
| `PATCH /settings/integrations/google_drive/configs/:document_type_id` | `#update` | Save Drive folder config |
| `GET/PATCH/DELETE /settings/integrations/zoho_drive` | `Settings::Integrations::ZohoDriveController` | Zoho Drive integration |
| `GET /settings/integrations/calendars` | `Settings::Integrations::CalendarsController#show` | Calendar connections overview |
| `GET…DELETE /settings/integrations/connections` | `Settings::Integrations::ConnectionsController` (CRUD) | Custom HTTP connections (Workflows) |

### `/api/app` endpoints to build

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET /api/app/settings/integrations` | All integration statuses (Notion, Drive, Zoho Drive, Calendar) | `IndexController` |
| `GET/PATCH /api/app/settings/integrations/notion` | Notion config | `NotionController` |
| `DELETE /api/app/settings/integrations/notion/workspaces/:id` | Disconnect Notion workspace | `#destroy` |
| `GET/DELETE /api/app/settings/integrations/google_drive` | Drive connection | `GoogleDriveController` |
| `POST /api/app/settings/integrations/google_drive/retry_failed` | Retry failed uploads | `#retry_failed` |
| `GET/PATCH /api/app/settings/integrations/google_drive/configs/:document_type_id` | Drive folder config per doc-type | `GoogleDriveConfigsController` |
| `GET /api/app/settings/integrations/google_drive/configs/:document_type_id/folders` | Browse Drive folders | `#browse_folders` |
| `GET/PATCH/DELETE /api/app/settings/integrations/zoho_drive` | Zoho Drive config | `ZohoDriveController` |
| `GET /api/app/settings/integrations/calendars` | Calendar connections | `CalendarsController` |
| `GET/POST/PATCH/DELETE /api/app/settings/integrations/connections` | Custom connection CRUD | `ConnectionsController` |

---

## AI & data

### Web routes

| Web route | Controller#action | Purpose |
|---|---|---|
| `GET /settings/ai` | `Settings::AiController#show` | AI overview (mode, model, provider) |
| `POST /settings/ai/switch_mode` | `#switch_mode` | Switch between Scout mode presets |
| `PATCH /settings/ai/embeddings` | `#embeddings` | Toggle semantic embeddings |
| `GET/PATCH /settings/ai_prompts/:purpose` | `Settings::AiPromptsController#index|edit|update` | Custom AI guidance per feature |
| `GET/PATCH /settings/data_privacy` | `Settings::DataPrivacyController#show|update` | Data retention and AI-processing preferences |
| `PATCH /settings/ai_adapters/:id` | `Settings::AiAdaptersController#update` | Configure an AI provider adapter |
| `POST /settings/ai_adapters` | `#create` | Add AI adapter |
| `DELETE /settings/ai_adapters/:id` | `#destroy` | Remove AI adapter |
| `PATCH /ai_configurations/:id` | `AiConfigurationsController#update` | (top-level) Update AI config for a document type or context |

### `/api/app` endpoints to build

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET /api/app/settings/ai` | AI overview (mode, provider, embeddings toggle) | `AiController` |
| `POST /api/app/settings/ai/switch_mode` | Switch mode preset | `#switch_mode` |
| `PATCH /api/app/settings/ai/embeddings` | Toggle embeddings | `#embeddings` |
| `GET /api/app/settings/ai_prompts` | List custom AI prompts by purpose | `AiPromptsController#index` |
| `GET/PATCH /api/app/settings/ai_prompts/:purpose` | Edit AI prompt | `#edit|update` |
| `GET/PATCH /api/app/settings/data_privacy` | Data privacy preferences | `DataPrivacyController` |
| `GET /api/app/settings/ai_adapters` | List AI adapters (verify: no index route today) | `AiAdaptersController` |
| `POST /api/app/settings/ai_adapters` | Add adapter | `#create` |
| `PATCH /api/app/settings/ai_adapters/:id` | Update adapter | `#update` |
| `DELETE /api/app/settings/ai_adapters/:id` | Remove adapter | `#destroy` |
| `PATCH /api/app/ai_configurations/:id` | Update AI config per context | `AiConfigurationsController` |

---

## Templates & pipelines

### Web routes

| Web route | Controller#action | Purpose |
|---|---|---|
| `GET…DELETE /settings/document_templates` | `Settings::DocumentTemplatesController` (CRUD + regenerate) | Document template management |
| `POST /settings/document_templates/:id/regenerate` | `#regenerate` | Re-generate template fields via AI |
| `GET…DELETE /settings/email_templates` | `Settings::EmailTemplatesController` (CRUD + regenerate) | Email template management |
| `POST /settings/email_templates/:id/regenerate` | `#regenerate` | Re-generate email template |
| `GET /document_templates/:id/fill` | `DocumentTemplatesController#fill` | Fill a document template (top-level) |
| `POST /document_templates/:id/preview` | `#preview` | Preview a filled template |
| `POST /document_templates/:id/send_email` | `#send_email` | Email a filled template |
| `GET /email_templates` | `EmailTemplatesController#index` | Composer template picker (top-level) |
| `GET /email_templates/:id/fill_form` | `#fill_form` | Variables fill form for composer |
| `POST /email_templates/:id/apply` | `#apply` | Apply template to open compose form |
| `GET…DELETE /settings/pipelines` | `Settings::PipelinesController` (CRUD) | Pipeline definitions |
| `GET /pipelines/:id/board` | `PipelineBoardController#index` | Pipeline kanban board |
| `POST /pipelines/:id/move` | `#move` | Move a card on the pipeline board |
| `GET/POST/DELETE /pipelines/:id/memberships` | `PipelineMembershipsController` | Add/remove items from pipeline board |

### `/api/app` endpoints to build

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET/POST/PATCH/DELETE /api/app/settings/document_templates` | Document template CRUD | `Settings::DocumentTemplatesController` |
| `POST /api/app/settings/document_templates/:id/regenerate` | AI-regenerate template | `#regenerate` |
| `GET /api/app/document_templates/:id/fill` | Fill form data | `DocumentTemplatesController#fill` |
| `POST /api/app/document_templates/:id/preview` | Preview JSON | `#preview` |
| `POST /api/app/document_templates/:id/send_email` | Email filled template | `#send_email` |
| `GET/POST/PATCH/DELETE /api/app/settings/email_templates` | Email template CRUD | `Settings::EmailTemplatesController` |
| `POST /api/app/settings/email_templates/:id/regenerate` | AI-regenerate | `#regenerate` |
| `GET /api/app/email_templates` | Composer picker list | `EmailTemplatesController#index` |
| `GET /api/app/email_templates/:id/fill_form` | Variables schema for composer | `#fill_form` |
| `POST /api/app/email_templates/:id/apply` | Apply to compose (returns prefill payload) | `#apply` |
| `GET/POST/PATCH/DELETE /api/app/settings/pipelines` | Pipeline CRUD | `Settings::PipelinesController` |
| `GET /api/app/pipelines/:id/board` | Pipeline board data | `PipelineBoardController` |
| `POST /api/app/pipelines/:id/move` | Move card | `#move` |
| `GET/POST/DELETE /api/app/pipelines/:id/memberships` | Board membership | `PipelineMembershipsController` |

---

## Notifications

Notifications have two separate route groups: the notification center (top-level
`resources :notifications`) and the notification preferences (top-level
`notification_preferences` routes). There is also a `resource :device` for native
push-token registration, and a `settings/notifications` page for digest preferences.

### Web routes

| Web route | Controller#action | Purpose |
|---|---|---|
| `GET /notifications` | `NotificationsController#index` | Notification center list |
| `GET /notifications/:id` | `#show` | Single notification |
| `DELETE /notifications/:id` | `#destroy` | Delete notification |
| `POST /notifications/:id/mark_read` | `#mark_read` | Mark one read |
| `POST /notifications/:id/archive` | `#archive` | Archive one |
| `POST /notifications/:id/unarchive` | `#unarchive` | Unarchive one |
| `POST /notifications/mark_all_read` | `#mark_all_read` | Mark all read |
| `POST /notifications/archive_all` | `#archive_all` | Archive all |
| `PATCH /notification_preferences/toggle` | `#toggle_preference` | Toggle a single notification category |
| `PATCH /notification_preferences/bulk_toggle` | `#bulk_toggle` | Bulk-toggle notification categories |
| `GET /settings/notifications` | `Settings::NotificationsController#index` | Notifications settings page |
| `PATCH /settings/notifications/digest_preference` | `#digest_preference` | Set email-digest frequency |
| `POST /device` | `DevicesController#create` | Register native push token (iOS/Android) |
| `DELETE /device` | `#destroy` | Unregister push token (sign-out/permission revoke) |
| `GET /settings/api_clients` | `Settings::ApiClientsController#index|new|create|destroy` | Developer API client management |
| `POST /settings/api_clients/:id/regenerate_secret` | `#regenerate_secret` | Rotate client secret |
| `POST /settings/api_clients/:id/revoke` | `#revoke` | Revoke a client |

### `/api/app` endpoints to build

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET /api/app/notifications` | Notification center list (paginated) | `NotificationsController#index` |
| `GET /api/app/notifications/:id` | Single notification | `#show` |
| `DELETE /api/app/notifications/:id` | Delete | `#destroy` |
| `POST /api/app/notifications/:id/mark_read` | Mark read | `#mark_read` |
| `POST /api/app/notifications/:id/archive` | Archive | `#archive` |
| `POST /api/app/notifications/:id/unarchive` | Unarchive | `#unarchive` |
| `POST /api/app/notifications/mark_all_read` | Mark all read | `#mark_all_read` |
| `POST /api/app/notifications/archive_all` | Archive all | `#archive_all` |
| `GET /api/app/notification_preferences` | List preferences with current state | (`Settings::NotificationsController`) |
| `PATCH /api/app/notification_preferences/toggle` | Toggle one category | `#toggle_preference` |
| `PATCH /api/app/notification_preferences/bulk_toggle` | Bulk toggle | `#bulk_toggle` |
| `PATCH /api/app/settings/notifications/digest_preference` | Digest frequency | `Settings::NotificationsController#digest_preference` |
| `POST /api/app/device` | Register native push token | `DevicesController#create` |
| `DELETE /api/app/device` | Unregister push token | `#destroy` |
| `GET /api/app/settings/api_clients` | List API clients | `Settings::ApiClientsController` |
| `POST /api/app/settings/api_clients` | Create client | `#create` |
| `DELETE /api/app/settings/api_clients/:id` | Delete client | `#destroy` |
| `POST /api/app/settings/api_clients/:id/regenerate_secret` | Rotate secret (returns secret once) | `#regenerate_secret` |
| `POST /api/app/settings/api_clients/:id/revoke` | Revoke client | `#revoke` |

---

## Read models / serializers needed

- `Api::App::AccountSerializer` — user profile + prefs, no sensitive fields.
- `Api::App::SecuritySerializer` — enrolled 2FA methods + active sessions + audit events.
- `Api::App::WorkspaceSerializer` — workspace fields + plan + member count.
- `Api::App::MemberSerializer` — user + role + invitation status.
- `Api::App::NotificationSerializer` — matches `Notification` model (kind, body, read, archived, created_at, action_url).
- `Api::App::TagSerializer`, `Api::App::RuleSerializer`, `Api::App::DocumentTypeSerializer`, `Api::App::SignatureSerializer` — inbox-settings resources.
- `Api::App::IntegrationStatusSerializer` — per-provider connection status (connected, last_synced_at, error).
- `Api::App::MemoryEntrySerializer` — Scout memory entry sentence + kind (taught/learned/default) + stable id.

## Open questions

- **Secret-once UX for API clients:** the web shows the secret in a modal immediately after
  creation. The SPA needs the same; consider returning `{ data: { secret } }` in the `POST`
  response and never again (same as today's `#create` flash).
- **Export download:** today it's a Rails `send_data` redirect. For the SPA, return a
  signed short-lived URL so the download can open in a new tab or trigger natively.
- **Push notifications:** `resource :device` is the registration surface, but the actual
  push delivery (APNs/FCM) is not wired in this codebase (verify: `DevicesController`
  stores the token; actual send logic may be absent or in `campbooks-cloud`).
- **Integrations OAuth flows** (Google Drive, Notion, Zoho) need the same native deep-link
  handoff pattern as auth (see `00-auth.md`). Settings integration connect/disconnect is a
  separate OAuth flow per provider, not the login flow — the SPA must handle the redirect
  back from the provider without breaking out to a full-page reload.
- **`AiAdaptersController` index** — no `GET /settings/ai_adapters` route exists today
  (only create/update/destroy). The SPA will need a list endpoint; confirm whether the `ai`
  show action already includes adapter state.
