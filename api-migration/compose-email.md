# Email reading, Compose + AI Compose & Account management — `/api/app`

**Status:** ⬜ not started · **Priority:** P2 · **Depends on:** [`00-auth.md`](00-auth.md)

This tracker covers: reading email messages and threads (message/thread views, drawer, board),
composing new email (compose-chat, AI rewrite, scheduled send), draft autosave + inline uploads,
and connecting + managing email/IMAP accounts. The People surface (standings, lanes, stand-note)
is in [`people-now.md`](people-now.md) and shares the thread/message read models with this surface.

## What the SPA must render

**Message/thread view** — thread list (folder/group scoped, infinite scroll, search); thread
detail with message chain (newest open, older folded), each message carrying Reply / Reply all /
Forward chips, read/unread toggle, star, archive, todo/follow-up dismiss, folder assignment,
tag chips, event-draft creation; linked Notion export; inline comment thread (email comments);
Board view (status kanban, `ENABLE_EMAIL_BOARD`-gated).

**Compose + AI** — compose dock: To/CC/BCC, Subject, rich-text body, attachment upload,
inline image upload, From-account picker, scheduled-send toggle, signature. AI compose-chat
(Scout drives the draft): chat turn → `EmailComposeChatController` → streaming Scout reply
updates the draft body. Tone rewrite (`Shorter` / `Warmer` / etc.): `POST rewrite_draft` →
`Ai::DraftRewriter#rewrite`. Intent prefill: `?intent=`/`?to=` params → `Emails::IntentPrefill`
infers To + Subject shown as "· inferred" chips. Draft autosave: `DraftEmail` record,
created/updated on each pause, dismissed/undismissed explicitly.

**Email accounts** — connected account list (OAuth + IMAP), per-account status (sync state,
last scan), sharing panel (add/remove users + permission level), IMAP account
connect/re-auth form, email folder reorder, scan history.

## Web routes being replaced

| Web route | Controller#action | Notes |
|---|---|---|
| `GET /email_messages` | `email_messages#index` | thread list; params: folder, group, page, format |
| `GET /email_messages/search` | `email_messages#search` | full-text search |
| `GET /email_messages/:id` | `email_messages#show` | message detail + thread chain |
| `GET /email_messages/:id/drawer_content` | `email_messages#drawer_content` | side-panel fragment |
| `GET /email_messages/:id/folders` | `email_messages/folders#index` | folder assignment picker |
| `POST /email_messages/:id/tool` | `email_tools#create` | single-email EmailAction dispatch |
| `PATCH /email_messages/:id/dismiss_todo` | `email_messages#dismiss_todo` | |
| `POST /email_messages/:id/dismiss_follow_up` | `email_messages#dismiss_follow_up` | |
| `GET /email_messages/:id/event_draft` | `email_messages/event_drafts#show` | |
| `POST /email_messages/:id/event_draft` | `email_messages/event_drafts#create` | `Ai::EventExtractor` |
| `POST /email_messages/:id/follow` | `thread_follows#create` | |
| `DELETE /email_messages/:id/follow` | `thread_follows#destroy` | |
| `POST /email_messages/:id/compose` | `email_compose#create` | open compose dock for reply/forward |
| `POST /email_messages/:id/send_message` | `email_compose#send_message` | send reply/forward; `Emails::Sender` |
| `POST /email_messages/:id/discard_compose` | `email_compose#discard` | discard the compose dock |
| `POST /email_messages/:id/notion_exports` | (notion export) | via `Documents::NotionExportsController` |
| `POST /email_messages/:id/comments` | `email_comments#create` | |
| `GET /email_messages/:id/comments/poll` | `email_comments#poll` | long-poll for new comments |
| `POST /email_messages/:id/tags` | `email_message_tags#create` | |
| `DELETE /email_messages/:id/tags/:id` | `email_message_tags#destroy` | |
| `GET /email_messages/new` | `email_messages#new` | compose-new surface (Desk) |
| `POST /email_messages/compose_chat` | `email_compose_chat#create` | Scout compose-chat turn |
| `POST /email_messages/rewrite_draft` | `email_compose#rewrite` | AI tone rewrite |
| `POST /email_messages/send_new` | `email_compose#send_message` | send a brand-new message |
| `POST /email_messages/compose_new` | `email_compose#create` | open new compose dock |
| `GET /email_messages/board` | `email_messages/board#index` | board (kanban) view |
| `POST /email_messages/board_move` | `email_messages/board#move` | move card between columns |
| `POST /email_messages/bulk` | `email_messages/bulk#create` | bulk action dispatch; `Emails::BulkActions` |
| `POST /compose_images` | `compose_images#create` | inline image upload for body |
| `POST /compose_attachments` | `compose_attachments#create` | file attachment upload |
| `GET /draft_emails/:id` | `draft_emails#show` | |
| `POST /draft_emails` | `draft_emails#create` | |
| `PATCH /draft_emails/:id` | `draft_emails#update` | autosave |
| `DELETE /draft_emails/:id` | `draft_emails#destroy` | |
| `POST /draft_emails/:id/dismiss` | `draft_emails#dismiss` | mark dismissed |
| `POST /draft_emails/:id/undismiss` | `draft_emails#undismiss` | |
| `GET /email_threads` | `email_threads#index` | |
| `GET /email_threads/:id` | `email_threads#show` | |
| `POST /email_accounts` | `email_accounts#create` | OAuth connect kickoff (redirect flow) |
| `PATCH /email_accounts/:id` | `email_accounts#update` | settings / signature / sync toggle |
| `DELETE /email_accounts/:id` | `email_accounts#destroy` | disconnect |
| `GET /email_accounts/:id/sharing` | `email_accounts#sharing` | sharing panel |
| `GET /email_accounts/:id/popover` | `email_accounts#popover` | quick-status popover |
| `GET /imap_accounts/new` | `imap_accounts#new` | IMAP connect form |
| `POST /imap_accounts` | `imap_accounts#create` | IMAP connect; `Imap::MailClient#verify!` |
| `GET /imap_accounts/:id/edit` | `imap_accounts#edit` | |
| `PATCH /imap_accounts/:id` | `imap_accounts#update` | |
| `PATCH /email_accounts/:id/email_folders/reorder` | `email_folders#reorder` | |
| `GET /email_scans/:id` | `email_scans#show` | scan audit trail |

## `/api/v1` coverage today

| Resource | v1 status | Gap |
|---|---|---|
| Emails (index/show/create) | ✅ `Api::V1::EmailMessagesController` | No drawer, board, bulk-action names, dismiss-todo, event-draft, folder-picker |
| mark_read / mark_unread | ✅ v1 member actions | |
| reply | ✅ v1 member `reply`; uses `Emails::Sender` | No compose-chat, rewrite, intent-prefill |
| Email actions (archive, star, etc.) | ✅ `email_actions#create` → `EmailActions.run` | |
| Bulk actions | ✅ `email_bulk#create` → `Emails::BulkActions.call` | |
| Email tags | ✅ `email_tags#create/destroy` | |
| Email threads (index/show) | 🟡 `Api::V1::EmailThreadsController` — raw resource | No follow, thread-chain shape |
| Drafts (index/show/create/update/destroy) | ✅ `Api::V1::DraftsController` | No dismiss/undismiss, no autosave intent, no inline-upload attachment linking |
| Email templates (index/show/create/update/destroy/apply) | ✅ `Api::V1::EmailTemplatesController` | |
| Folders (index/show) | ✅ `Api::V1::FoldersController` | No IMAP folder reorder |
| Folder memberships | ✅ `Api::V1::FolderMembershipsController` | |
| Compose-chat (AI Scout drafting) | ❌ | Absent from v1 |
| Draft rewrite (tone: shorter/warmer) | ❌ | `email_compose#rewrite` / `Ai::DraftRewriter` not in v1 |
| Compose image/attachment upload | ❌ | `ComposeImagesController`, `ComposeAttachmentsController` not in v1 |
| Email account connect/manage | ❌ | Absent from v1 |
| IMAP account connect/manage | ❌ | Absent from v1 |
| Email Skim | ❌ | Absent (see `paper-files.md`) |
| Email comments (poll) | ❌ | Absent from v1 |
| Board view | ❌ | Absent from v1 |
| Intent-prefill | ❌ | `Emails::IntentPrefill` not in v1 |

## `/api/app` endpoints to build

### Email reading — messages & threads

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET /api/app/email_messages` | Thread-list for a folder/group; paginated | `EmailMessagesController#index`; `Emails::TagGroups`, `Emails::InboxFolders` |
| `GET /api/app/email_messages/search` | Full-text + folder-scoped message search | `EmailMessagesController#search`; `Emails::Search`, `Emails::SearchQuery` |
| `GET /api/app/email_messages/:id` | Message detail — headers, body, attachments, thread chain | `EmailMessagesController#show`; `Emails::MarkThreadRead` (`app/services/emails/mark_thread_read.rb`) |
| `GET /api/app/email_messages/:id/folders` | Folder-assignment picker data | `email_messages/folders#index` |
| `POST /api/app/email_messages/:id/dismiss_todo` | Dismiss the to-do chip | `EmailMessagesController#dismiss_todo` |
| `POST /api/app/email_messages/:id/dismiss_follow_up` | Dismiss follow-up flag | `EmailMessagesController#dismiss_follow_up` |
| `GET /api/app/email_messages/:id/event_draft` | AI-draft a calendar event from the email | `email_messages/event_drafts#show`; `Ai::EventExtractor` |
| `POST /api/app/email_messages/:id/event_draft` | Create the event from the draft | `email_messages/event_drafts#create`; `Tools::CreateCalendarEvent` |
| `POST /api/app/email_messages/:id/follow` | Follow the thread | `thread_follows#create` |
| `DELETE /api/app/email_messages/:id/follow` | Unfollow | `thread_follows#destroy` |
| `POST /api/app/email_messages/:id/tool` | Dispatch a single-email `EmailActions` tool | `email_tools#create`; `EmailActions.run` (`app/services/email_actions.rb`) |
| `POST /api/app/email_messages/:id/tags` | Add a tag | `email_message_tags#create` |
| `DELETE /api/app/email_messages/:id/tags/:tag_id` | Remove a tag | `email_message_tags#destroy` |
| `POST /api/app/email_messages/:id/comments` | Create an email comment | `email_comments#create` |
| `GET /api/app/email_messages/:id/comments` | Poll for new comments (replaces long-poll) | `email_comments#poll` (convert to paginated list) |
| `GET /api/app/email_messages/board` | Board view columns + cards | `email_messages/board#index` |
| `POST /api/app/email_messages/board_move` | Move a card between board columns | `email_messages/board#move` |
| `POST /api/app/email_messages/bulk` | Bulk action on selected messages | `email_messages/bulk#create`; `Emails::BulkActions.call` (`app/services/emails/bulk_actions.rb`) |
| `GET /api/app/email_threads` | Thread list | `email_threads#index` (web); `Api::V1::EmailThreadsController` |
| `GET /api/app/email_threads/:id` | Thread detail with message chain | `email_threads#show`; `Api::V1::ThreadSerializer` |

### Compose — new message + reply/forward

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET /api/app/email_messages/compose_prefill` | Intent-prefill (To + Subject inference from `?intent=`/`?to=`) | `Emails::IntentPrefill.for(user:, intent:, to:)` (`app/services/emails/intent_prefill.rb`) |
| `POST /api/app/email_messages` | Open/create a compose session (new or reply/forward) | `EmailComposeController#create`; `Emails::ComposePrefill` |
| `POST /api/app/email_messages/send` | Send a new message | `EmailComposeController#send_message`; `Emails::Sender` (`app/services/emails/sender.rb`) |
| `POST /api/app/email_messages/:id/reply` | Send a reply or forward | `EmailComposeController#send_message`; `Emails::Sender` |
| `POST /api/app/email_messages/:id/discard_compose` | Discard an open compose session | `EmailComposeController#discard` |
| `POST /api/app/email_messages/rewrite_draft` | AI tone rewrite (Shorter / Warmer / …) | `EmailComposeController#rewrite`; `Ai::DraftRewriter#rewrite(body_html, tone:)` (`app/services/ai/draft_rewriter.rb`) |
| `POST /api/app/compose_chat` | Scout AI compose-chat turn; returns streaming reply that updates the draft | `EmailComposeChatController#create`; Scout tool pipeline |
| `POST /api/app/compose_images` | Inline image upload for body (base64 or multipart) | `ComposeImagesController#create` (`app/controllers/compose_images_controller.rb`) |
| `POST /api/app/compose_attachments` | File attachment upload | `ComposeAttachmentsController#create` (`app/controllers/compose_attachments_controller.rb`) |

### Drafts

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET /api/app/drafts` | Draft list | `Api::V1::DraftsController#index`; `Api::V1::DraftSerializer` |
| `GET /api/app/drafts/:id` | Draft detail | `Api::V1::DraftsController#show` |
| `POST /api/app/drafts` | Create draft (autosave) | `DraftEmailsController#create`; `Api::V1::DraftsController#create` |
| `PATCH /api/app/drafts/:id` | Update draft (autosave debounce) | `DraftEmailsController#update`; `Api::V1::DraftsController#update` |
| `DELETE /api/app/drafts/:id` | Discard draft | `DraftEmailsController#destroy`; `Api::V1::DraftsController#destroy` |
| `POST /api/app/drafts/:id/dismiss` | Dismiss draft reminder | `DraftEmailsController#dismiss` |
| `POST /api/app/drafts/:id/undismiss` | Un-dismiss | `DraftEmailsController#undismiss` |

### Email account connect & management

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET /api/app/email_accounts` | List connected accounts (status, last scan, provider) | `EmailAccountsController` (extract list logic) |
| `PATCH /api/app/email_accounts/:id` | Update account settings (display name, sync toggle, signature) | `EmailAccountsController#update` |
| `DELETE /api/app/email_accounts/:id` | Disconnect account | `EmailAccountsController#destroy` |
| `GET /api/app/email_accounts/:id/sharing` | Sharing panel data (users + roles) | `EmailAccountsController#sharing` |
| `PATCH /api/app/email_accounts/:id/sharing` | Add/update sharing member | `EmailAccountsController#update` (sharing path) |
| `GET /api/app/email_accounts/:id/popover` | Quick-status popover data | `EmailAccountsController#popover` |
| `PATCH /api/app/email_accounts/:id/email_folders/reorder` | Reorder inbox label folders | `EmailFoldersController` (web) |
| `GET /api/app/email_scans/:id` | Scan audit-trail entry | `EmailScansController#show` |
| `POST /api/app/imap_accounts` | Connect an IMAP account (credentials + verify) | `ImapAccountsController#create`; `Imap::MailClient#verify!` |
| `PATCH /api/app/imap_accounts/:id` | Update IMAP credentials / host | `ImapAccountsController#update` |

> ⚠️ **OAuth connect is a redirect flow.** `POST /email_accounts` kicks off a
> provider OAuth round-trip (Zoho/Google/Microsoft), ending at
> `/oauth/{zoho,gmail,microsoft}/callback`. This cannot be fully JSON-ified without an
> in-app browser or deep-link handoff. For Capacitor, the existing `OauthNativeHandoff`
> pattern from [`00-auth.md`](00-auth.md) applies: open the provider URL in the system
> browser → deep-link back → exchange one-time token. The JSON API needs only the
> "what accounts do I have" read endpoint and the settings mutations above; the connect
> *kickoff* remains a redirect. See **Open questions** below.

## Read models / serializers needed

- `Api::App::EmailMessageSerializer` — extend `Api::V1::EmailSerializer` (`app/serializers/api/v1/email_serializer.rb`) with: thread chain (messages oldest-to-newest), body HTML, attachments with download URLs, folder memberships, tag chips, follow-up state, dismiss-todo state, compose-dock state, event-draft stub.
- `Api::App::ThreadSerializer` — extend `Api::V1::ThreadSerializer` with: message stubs (sender, snippet, read, date), folder, unread count, follow state, people-permalink.
- `Api::App::ComposeSessionSerializer` — transient compose state: draft id, to/cc/bcc inferred chips, subject, body HTML, attachment entries, from-account id, Scout draft mark.
- `Api::App::DraftSerializer` — can reuse `Api::V1::DraftSerializer` (`app/serializers/api/v1/draft_serializer.rb`); add `dismissed` flag.
- `Api::App::EmailAccountSerializer` — account id, provider, address, display_name, sync_enabled, last_scan_at, push_watch_expires_at, permissions for current user.
- `Api::App::ComposeRewriteSerializer` — `{ body_html, tone }` round-trip shape from `Ai::DraftRewriter`.
- `Api::App::IntentPrefillSerializer` — `{ to, subject, to_inferred, subject_inferred }` from `Emails::IntentPrefill`.

## Real-time

| Channel | What it carries | Today's mechanism |
|---|---|---|
| `inbox_<user_id>` | New message arrival, read/unread, archive, thread row update | `Emails::InboxBroadcaster` → `Turbo::StreamsChannel.broadcast_*_to` per user — needs JSON equivalent event |
| `people_<user_id>` | People-lane row updates (shared with People surface) | Same broadcaster; see [`people-now.md`](people-now.md) |
| `compose_dock_<user_id>` | Scout draft streamed into compose body | `EmailComposeChatController` → Turbo stream today — needs SSE or Action Cable JSON event for SPA streaming |
| `thread_<thread_id>` | New incoming message appended to open thread; comment created | `Emails::InboxBroadcaster#broadcast_thread` (verify) |

The inbox broadcaster (`app/services/emails/inbox_broadcaster.rb`) currently emits Turbo HTML
fragments. The JSON equivalent is a `inbox_updated` event on an Action Cable JSON channel
carrying enough data to patch the TanStack Query cache (thread row + unread count delta).
See [`realtime.md`](realtime.md) for the cross-cutting channel design.

The compose-chat streaming path today pushes Turbo stream HTML back to the compose dock.
For the React SPA, the streaming response should be SSE or a WebSocket message carrying
delta text, so the client can stream the draft body in real time.

## Open questions

1. **OAuth connect in Capacitor**: `EmailAccountsController#create` is a form POST that
   immediately redirects to the provider. For the SPA/Capacitor flow, decide whether to:
   (a) open the OAuth URL in an in-app browser (Capacitor Browser plugin) with the
   `OauthNativeHandoff` deep-link return, or (b) open in the system browser (same as auth
   sign-in). Both paths reuse existing redirect URIs — no provider-console change needed.
   Call out in the account-connect UI that connect is a redirect, not a JSON round-trip.
2. **Compose-chat streaming**: the current `EmailComposeChatController` pushes Turbo stream
   updates synchronously. For the React SPA, decide on SSE (`text/event-stream`) vs Action
   Cable for streaming Scout draft tokens. SSE is simpler and stateless; Action Cable matches
   the existing real-time channel pattern.
3. **Board view gate**: the board surface is behind `Features.workflows?` today — confirm
   whether `ENABLE_EMAIL_BOARD` is the correct gate and whether the SPA should show the board
   tab at all when the flag is off.
4. **Email comments vs threads**: `email_comments` today use a poll (`GET …/comments/poll`)
   for new comments. For the SPA, a short-poll endpoint is viable; a push channel is better
   long-term. Align with the real-time design in [`realtime.md`](realtime.md).
5. **IMAP account form validation**: `Imap::MailClient#verify!` is called synchronously on
   `ImapAccountsController#create` to live-verify credentials before saving. This blocks the
   request for the IMAP handshake duration. For the JSON API, consider returning a 202 +
   a poll/webhook for verification result, or accept the synchronous pattern with an
   appropriate timeout.
6. **Draft autosave debounce**: the SPA must debounce PATCH calls to avoid thundering-herd
   on every keystroke. Confirm whether the API should support conditional PUT with
   `If-Unmodified-Since` / ETags, or rely on client-side debounce alone.
