# Paper, Files, Document Skim, Email Skim & Search — `/api/app`

**Status:** ⬜ not started · **Priority:** P2 · **Depends on:** [`00-auth.md`](00-auth.md)

Paper is the document inbox (receipts, invoices, contracts, filings — each reviewed in its
bucket). Files is the folder-based file manager (uploaded originals). Document Skim and Email
Skim are ring-deck review queues. Search is global Cmd+K. All five replace Hotwire surfaces
today served by `PaperController`, `FilesController`, `DocumentsController`,
`Documents::SkimController`, `SkimController`, and `SearchController`.

## What the SPA must render

**Paper** — tabbed bucket view (`incoming | invoices | receipts | contracts | filings | other`);
each bucket: document cards with extracted fields (amount, date, party), status chips
(pending/approved/rejected/settled), star, pagination. Per-document detail: full extracted
fields, AI summary, file preview (PDF/image), action bar (approve, reject, rename, settle,
push to Drive/Notion/Zoho Drive, reprocess, merge, export). Written-document sub-surface
(`documents/write`): create/edit long-form docs. Notion/Drive export flows.

**Files** — folder tree sidebar + file grid; upload files (immediate + AI-analyze option);
public link generation and revocation; per-folder share panel; filed email preview.
Mail-folder CRUD (inbox label folders) and folder membership management.

**Document Skim** — ring deck (`incoming`, `invoices`, `contracts`, …); per-card: approve,
reclassify (change document type + re-extract fields), update fields inline, reprocess, dismiss,
restore. Tray shows next card on action.

**Email Skim** — ring deck by sender category; per-card: decide (archive/done/snooze/star),
keep (unsubscribe-style), promote, unpromote, dismiss follow-up, sender-level actions
(block/allow), inline email preview, inline reply. Tray advances on every action.

**Search** — Cmd+K global: emails, documents, contacts, threads, tags, document types,
workflows; semantic (embedding) + ilike fallback; grouped results with type icons and
subtitles. (`GlobalSearch.call` — verified `app/services/global_search.rb`).

**Organizations** — org detail: people list, email thread list, document list, backfill action.

## Web routes being replaced

| Web route | Controller#action | Notes |
|---|---|---|
| `GET /paper` | `paper#index` | bucket index; params `q`, `type`, `status`, `page` |
| `GET /documents` | redirects → `/files` | replaced by Files |
| `GET /documents/:id` | `documents#show` | detail page |
| `PATCH /documents/:id` | `documents#update` | field edits |
| `POST /documents` | `documents#create` | upload |
| `GET /documents/:id/file` | `documents#file` | serve raw file |
| `PATCH /documents/:id/rename` | `documents#rename` | |
| `POST /documents/:id/approve` | `documents#approve` | |
| `POST /documents/:id/reject` | `documents#reject` | |
| `PATCH /documents/:id/toggle_star` | `documents#toggle_star` | |
| `POST /documents/:id/reprocess` | `documents#reprocess` | |
| `POST /documents/:id/settle` | `documents#settle` | |
| `DELETE /documents/:id/settle` | `documents#unsettle` | |
| `POST /documents/:id/push_to_notion` | `documents#push_to_notion` | Integrations::Notion::PageCreator |
| `POST /documents/:id/push_to_drive` | `documents#push_to_drive` | Integrations::Drive::FileUploader |
| `POST /documents/:id/push_to_zoho_drive` | `documents#push_to_zoho_drive` | |
| `POST /documents/reprocess_all` | `documents#reprocess_all` | |
| `POST /documents/export` | `documents#export` | CSV/PDF bulk export |
| `GET /documents/merge` | `documents#merge` | merge picker |
| `POST /documents/perform_merge` | `documents#perform_merge` | |
| `GET /documents/write` | `documents/written#index` | written docs list |
| `GET /documents/write/new` | `documents/written#new` | |
| `POST /documents/write` | `documents/written#create` | |
| `GET /documents/write/:id` | `documents/written#show` | |
| `GET /documents/write/:id/edit` | `documents/written#edit` | |
| `PATCH /documents/write/:id` | `documents/written#update` | |
| `GET /documents/skim` | `documents/skim#show` | deck surface |
| `GET /documents/skim/tray` | `documents/skim#tray` | next-card fragment |
| `POST /documents/skim/:id/approve` | `documents/skim#approve` | |
| `PATCH /documents/skim/:id/reclassify` | `documents/skim#reclassify` | |
| `PATCH /documents/skim/:id/update_fields` | `documents/skim#update_fields` | |
| `POST /documents/skim/:id/reprocess` | `documents/skim#reprocess` | |
| `POST /documents/skim/:id/dismiss` | `documents/skim#dismiss` | |
| `POST /documents/skim/:id/restore` | `documents/skim#restore` | |
| `GET /files` | `files#index` | folder root |
| `GET /files/folders/:id` | `files#show` | folder contents |
| `POST /files/uploads` | `files/uploads#create` | raw file upload |
| `DELETE /files/uploads/:id` | `files/uploads#destroy` | |
| `POST /files/uploads/:id/analyze` | `files/uploads#analyze` | trigger AI analysis |
| `POST /files/folders/:id/share` | (per-folder share) | |
| `POST /files/public_links` | `files/public_links#create` | |
| `DELETE /files/public_links/:id` | `files/public_links#destroy` | |
| `GET /files/public_links/picker` | `files/public_links#picker` | |
| `GET /f/:token` | `public_files#show` | unauthenticated public file |
| `GET /skim` | `skim#show` | email skim deck |
| `GET /skim/tray` | `skim#tray` | |
| `POST /skim/decide` | `skim#decide` | |
| `POST /skim/undo` | `skim#undo` | |
| `POST /skim/keep` | `skim#keep` | |
| `POST /skim/promote` | `skim#promote` | |
| `POST /skim/unpromote` | `skim#unpromote` | |
| `POST /skim/dismiss_follow_up` | `skim#dismiss_follow_up` | |
| `POST /skim/sender_action` | `skim#sender_action` | |
| `GET /skim/email/:id` | `skim#email` | email card preview |
| `GET /skim/email/:id/content` | `skim#email_content` | body fragment |
| `POST /skim/email/:id/reply` | `skim#reply` | inline reply |
| `GET /search` | `search#index` | GlobalSearch |
| `GET /organizations` | `organizations#index` | |
| `GET /organizations/:id` | `organizations#show` | |
| `PATCH /organizations/:id` | `organizations#update` | |
| `GET /organizations/:id/emails` | `organizations#emails` | |
| `GET /organizations/:id/documents` | `organizations#documents` | |
| `POST /organizations/backfill` | `organizations#backfill` | |
| `GET/PATCH /mail_folders/:id` | `mail_folders#*` | inbox label folders |
| `POST/DELETE /folder_memberships` | `folder_memberships#*` | filing |

## `/api/v1` coverage today

| Resource | v1 status | Gap |
|---|---|---|
| Documents CRUD | 🟡 index/show/create/update + file/approve/reject/reclassify | No Paper buckets, settle/unsettle, push-to-Drive/Notion, merge, star, rename, written docs, skim |
| Folders | ✅ index/show | No Files surface aggregation |
| Folder memberships | ✅ create/destroy | |
| Files manager | ❌ | Entirely absent |
| Public links | ❌ | Absent |
| Document Skim | ❌ | Absent |
| Email Skim | ❌ | Absent |
| Search | ❌ | Absent |
| Organizations | ❌ | Absent |

## `/api/app` endpoints to build

### Paper

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET /api/app/paper` | Paper index — bucket counts + current bucket's paginated docs | `PaperController#index` logic; `Documents::Filters`, `Documents::Sorter` (verify) |
| `GET /api/app/documents/:id` | Document detail with extracted fields, AI summary, source context | `Api::V1::DocumentSerializer` + extensions |
| `POST /api/app/documents` | Upload one or more documents | `DocumentsController#create`; `Documents::Processor` (`app/services/documents/processor.rb`) |
| `PATCH /api/app/documents/:id` | Edit extracted fields | `DocumentsController#update` |
| `PATCH /api/app/documents/:id/rename` | Rename | `DocumentsController#rename` |
| `POST /api/app/documents/:id/approve` | Approve | `DocumentsController#approve` |
| `POST /api/app/documents/:id/reject` | Reject | `DocumentsController#reject` |
| `PATCH /api/app/documents/:id/toggle_star` | Star/unstar | `DocumentsController#toggle_star` |
| `POST /api/app/documents/:id/reprocess` | Trigger AI re-extraction | `DocumentsController#reprocess` |
| `POST /api/app/documents/:id/settle` | Mark settled | `DocumentsController#settle` |
| `DELETE /api/app/documents/:id/settle` | Unsettle | `DocumentsController#unsettle` |
| `POST /api/app/documents/:id/push_to_drive` | Send to Google Drive | `Integrations::FileSource` + `Integrations::Drive::FileUploader` |
| `POST /api/app/documents/:id/push_to_notion` | Send to Notion | `Integrations::FileSource` + `Integrations::Notion::PageCreator` or `DatabaseItemCreator` |
| `POST /api/app/documents/:id/push_to_zoho_drive` | Send to Zoho WorkDrive | `DocumentsController#push_to_zoho_drive` (verify Zoho client) |
| `POST /api/app/documents/reprocess_all` | Bulk re-extraction | `DocumentsController#reprocess_all` |
| `POST /api/app/documents/export` | Bulk CSV/PDF export | `DocumentsController#export` |
| `GET /api/app/documents/merge` | Merge candidates | `DocumentsController#merge` |
| `POST /api/app/documents/perform_merge` | Commit merge | `DocumentsController#perform_merge` |
| `GET /api/app/documents/write` | Written docs list | `Documents::WrittenController#index` |
| `POST /api/app/documents/write` | Create written doc | `Documents::WrittenController#create` |
| `GET /api/app/documents/write/:id` | Written doc show | `Documents::WrittenController#show` |
| `PATCH /api/app/documents/write/:id` | Update written doc | `Documents::WrittenController#update` |
| `GET /api/app/documents/:id/file` | Serve/redirect to raw file | `DocumentsController#file` |

### Document Skim

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET /api/app/document_skim` | Current skim deck state (rings + top card) | `Documents::SkimController#show`; `Documents::SkimBuilder` (`app/services/documents/skim_builder.rb`) |
| `GET /api/app/document_skim/tray` | Next card after an action | `Documents::SkimController#tray`; `Documents::SkimTrayBroadcaster` (verify) |
| `POST /api/app/document_skim/:id/approve` | Approve card | `Documents::SkimController#approve` |
| `PATCH /api/app/document_skim/:id/reclassify` | Reclassify + re-extract | `Documents::SkimController#reclassify` |
| `PATCH /api/app/document_skim/:id/update_fields` | Field edits without re-classify | `Documents::SkimController#update_fields` |
| `POST /api/app/document_skim/:id/reprocess` | Queue AI re-extraction | `Documents::SkimController#reprocess` |
| `POST /api/app/document_skim/:id/dismiss` | Skip document | `Documents::SkimController#dismiss` |
| `POST /api/app/document_skim/:id/restore` | Restore a dismissed doc | `Documents::SkimController#restore` |

### Email Skim

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET /api/app/email_skim` | Email skim deck (rings + top card) | `SkimController#show`; `Emails::SkimBuilder` (`app/services/emails/skim_builder.rb`) |
| `GET /api/app/email_skim/tray` | Next card | `SkimController#tray`; `Emails::SkimTrayBroadcaster` |
| `POST /api/app/email_skim/decide` | Archive/done/snooze/star decision on top card | `SkimController#decide`; `Emails::SkimDecisionRecorder`, `Emails::SkimActionMemory` |
| `POST /api/app/email_skim/undo` | Undo last decision | `SkimController#undo` |
| `POST /api/app/email_skim/keep` | Keep (unsubscribe-style) | `SkimController#keep`; `Emails::SkimArchive` (verify) |
| `POST /api/app/email_skim/promote` | Promote sender to person | `SkimController#promote`; `Emails::SkimPromote` |
| `POST /api/app/email_skim/unpromote` | Unpromote | `SkimController#unpromote`; `Emails::SkimUnpromote` |
| `POST /api/app/email_skim/dismiss_follow_up` | Clear follow-up flag on card | `SkimController#dismiss_follow_up` |
| `POST /api/app/email_skim/sender_action` | Block/allow sender from skim | `SkimController#sender_action`; `EmailActions` |
| `GET /api/app/email_skim/emails/:id` | Email card body (lazy load) | `SkimController#email` + `#email_content` |
| `POST /api/app/email_skim/emails/:id/reply` | Inline reply from skim | `SkimController#reply`; `Emails::Sender` |

### Files

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET /api/app/files` | Root folder tree + contents | `FilesController#index` |
| `GET /api/app/files/folders/:id` | Folder detail (files + sub-folders) | `FilesController#show` |
| `POST /api/app/files/uploads` | Upload file(s) to a folder | `Files::UploadsController#create`; `Integrations::FileSource` |
| `DELETE /api/app/files/uploads/:id` | Remove an uploaded file | `Files::UploadsController#destroy` |
| `POST /api/app/files/uploads/:id/analyze` | Trigger AI analysis on upload | `Files::UploadsController#analyze`; `Documents::Processor` |
| `POST /api/app/files/public_links` | Generate public share link | `Files::PublicLinksController#create` |
| `DELETE /api/app/files/public_links/:id` | Revoke public link | `Files::PublicLinksController#destroy` |
| `GET /api/app/files/public_links/picker` | Picker UI data for public links | `Files::PublicLinksController#picker` |
| `GET /api/app/mail_folders/:id` | Mail/label folder detail | `MailFoldersController#show` |
| `POST /api/app/mail_folders` | Create label folder | `MailFoldersController#create` |
| `PATCH /api/app/mail_folders/:id` | Update label folder | `MailFoldersController#update` |
| `DELETE /api/app/mail_folders/:id` | Destroy label folder | `MailFoldersController#destroy` |
| `POST /api/app/folder_memberships` | File document into folder | `FolderMembershipsController#create` |
| `DELETE /api/app/folder_memberships/:id` | Remove from folder | `FolderMembershipsController#destroy` |

### Search

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET /api/app/search?q=&types[]=` | Global search — emails, docs, contacts, threads, tags, doc types, workflows | `SearchController#index`; `GlobalSearch.call(query, user:, types:)` (`app/services/global_search.rb`) |

### Organizations

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET /api/app/organizations` | Organization list | `OrganizationsController#index` |
| `GET /api/app/organizations/:id` | Org detail | `OrganizationsController#show` |
| `PATCH /api/app/organizations/:id` | Update org | `OrganizationsController#update` |
| `GET /api/app/organizations/:id/emails` | Org email threads | `OrganizationsController#emails` |
| `GET /api/app/organizations/:id/documents` | Org documents | `OrganizationsController#documents` |
| `POST /api/app/organizations/backfill` | Backfill org assignments | `OrganizationsController#backfill` |

## Read models / serializers needed

- `Api::App::PaperPageSerializer` — bucket counts + current-bucket page of `DocumentSerializer`; matches `PaperController`'s `bucket_counts` + `load_documents` result.
- `Api::App::DocumentSerializer` — extend `Api::V1::DocumentSerializer` (`app/serializers/api/v1/document_serializer.rb`) with settle state, star, source context (email subject + sender), push-to-Drive/Notion status, AI summary, and `skim_card` sub-hash for skim surfaces.
- `Api::App::DocumentSkimDeckSerializer` — rings array (category + cards) produced by `Documents::SkimBuilder`.
- `Api::App::EmailSkimDeckSerializer` — rings array produced by `Emails::SkimBuilder`, with each card carrying the email summary, sender, category, and action chips.
- `Api::App::FilesFolderSerializer` — folder tree entry (id, name, depth, counts) + paginated file entries (document_id, filename, size, content_type, public_link).
- `Api::App::SearchResultSerializer` — typed results (`{ type, title, subtitle, icon, id, url }`), grouped by type, from `GlobalSearch#call`.
- `Api::App::OrganizationSerializer` — org details, people count, recent email thread stubs, recent document stubs.

## Real-time

| Channel | What it carries | Today's mechanism |
|---|---|---|
| `document_skim_<user_id>` | Tray advance after approve/dismiss (next card HTML today) | `Documents::SkimTrayBroadcaster` → `Turbo::StreamsChannel.broadcast_replace_to` — needs JSON equivalent |
| `email_skim_<user_id>` | Tray advance after decide/keep/promote | `Emails::SkimTrayBroadcaster` → `Turbo::StreamsChannel.broadcast_replace_to` — needs JSON equivalent |
| `files_<workspace_id>` | New upload completed (AI-analyzed) | ❌ no channel today; polling or new channel needed |

Both skim broadcasters currently push Turbo HTML fragments. The JSON API equivalent is a
`skim_tray_updated` event on a per-user Action Cable channel carrying the next serialized card.
See [`realtime.md`](realtime.md) for the cross-cutting channel design.

## Open questions

1. **Public file URL for SPA**: `GET /f/:token` is served by `PublicFilesController < ActionController::Base` (no auth, `app/controllers/public_files_controller.rb`). The SPA can redirect to this URL; no `/api/app` twin needed unless native needs a pre-signed download URL instead.
2. **Skim deck cursor**: the current deck is server-scoped by the user's skim-scope and memory (rings + seen set). A stateless JSON API needs either a cursor token or a server-side deck session. Decide before implementing.
3. **Drive/Notion export UI**: the interactive "pick folder" flows (`documents/drive_exports`, `documents/notion_exports` with `get :databases`, `get :database_form`, `get :pages` member routes on documents) depend on multi-step modals. The picker endpoints need to be JSON-ified (`GET /api/app/drive/folders?parent=`, `GET /api/app/notion/databases`, etc.) — likely belongs in a `settings-integrations` tracker.
4. **Written documents**: `Documents::WrittenController` currently returns Hotwire/HTML; confirm whether the SPA will use a rich-text editor format (Trix JSON, ProseMirror, …) and align the `create`/`update` payload shape before implementing.
5. **Paper bucket redirect**: `GET /documents` redirects to `/files` today. Confirm the SPA should treat Paper (`/paper`) and Files (`/files`) as two separate nav destinations, each with their own `/api/app` endpoints.
