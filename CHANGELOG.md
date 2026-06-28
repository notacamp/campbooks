# Changelog

All notable changes to Campbooks are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
See [CONTRIBUTING.md](CONTRIBUTING.md#versioning--releases) for what counts as a
major, minor, or patch change here.

<!--
  Add your change under [Unreleased], in the matching group. Maintainers move it
  under a new version heading at release time.
  Groups, in order: Added · Changed · Deprecated · Removed · Fixed · Security.
  Pre-1.0: flag breaking changes under "Changed" with a ⚠️.
-->

## [Unreleased]

### Added

- **Email scheduling** — schedule one-time or recurring sends (daily, weekly,
  every 2 weeks, or monthly), gated by billing entitlements. Schedule a message
  from the composer's "Schedule for later" picker or the dedicated
  `/scheduled_emails` page; a per-minute job sends due messages and rolls
  recurring ones to their next occurrence. Snoozed threads and upcoming scheduled
  sends also surface on the calendar.
- **Organizations** — first-class company/employer grouping on top of contacts,
  gated by billing entitlements. People can belong to multiple organizations
  (active or past). Includes scoped filtering for emails and documents, a
  directory page, per-organization profile pages, and a backfill from existing
  AI-extracted `Person.organization` strings.
- **Sent-email attachments** — files attached to emails you send are now stored
  locally as documents (the same way received attachments are), and the AI
  biases their classification toward revenue/outgoing types.

### Changed

- Sidebar navigation attention dots now reflect whether a section still has
  something for you to look at — unread mail, new feed items, pending reminders,
  documents awaiting review, or unread Scout replies — and clear when you handle
  the resource from any surface (home feed, skim, mail, or Scout), rather than
  being tied to when you last opened that section's page.

### Fixed

- Avatar stacks (facepiles) in the email list and board view now show the
  account-color ring, consistent with single-sender avatars and search results.

## [0.2.1] - 2026-06-27

### Changed

- System labels imported from Gmail (IMPORTANT, CATEGORY_PERSONAL, etc.) now get
  human-readable names (e.g. "Personal"), a muted colour palette, and are **hidden
  by default** in the inbox tag list. Toggle them back on via the new
  `show_system_labels` workspace setting.
- Mistral is now available as a document-analysis (vision) provider
  (`pixtral-large-latest`), and is the managed default for EU residents.
  Document analysis now uses `pixtral-large-latest`; text AI continues on
  `mistral-small-latest`.

### Fixed

- **Document AI extraction blank in prod** — PDF documents processed through the
  OpenAI adapter were failing silently because ImageMagick was missing from the
  Docker image (`convert: command not found`). Added `imagemagick ghostscript` to
  the base image and relaxed the default PDF security policy so `convert` can read
  PDFs. Switched the managed "Campbooks AI" document provider from OpenAI to Mistral
  (EU-based, `pixtral-large-latest`), so managed document analysis now runs entirely
  on EU infrastructure.

## [0.2.0] - 2026-06-26

### Added

- **Auto-ingest document links from emails** — when an email body contains
  direct links to downloadable files (PDFs, Office documents, spreadsheets,
  CSVs), Campbooks now downloads them safely and creates Documents that flow
  through the same AI analysis pipeline as regular attachments. Fetching is
  SSRF-guarded (UrlGuard), content-type-verified, size-capped (25 MB),
  and per-link failures are isolated so one broken link never blocks the
  rest. Cloud-share links (Google Drive, Dropbox, WeTransfer) are deliberately
  skipped in this first iteration. \[#56\]
- **EU data residency** (Settings → Data & Privacy) — an opt-in workspace policy that
  restricts AI processing to EU-region providers. When on, only EU providers may be used:
  text AI continues on an EU provider (Mistral), while document AI and semantic search —
  which have no EU provider yet — **pause** rather than send data to a US provider. The
  page shows a "Paused" badge on each affected AI task so it's never silent.
- An opt-in **"Auto-delete old email"** setting (Settings → Data & Privacy) — choose a
  retention window for Campbooks' stored copy of your email (6/12/24/36 months), and the
  daily sweep permanently deletes our local copy (message, search index, and cached
  attachments) that's older than that window. **Your mailbox is never touched** — the
  original stays in your inbox; only Campbooks' copy is removed. Off by default.
- **Complete data export** (Settings → Account → "Your data") — your data export is now
  assembled in the background as a downloadable archive that includes your actual email
  content, attachments, and document files (not just a JSON summary). You're notified when
  it's ready. Replaces the previous inline JSON-only download.
- **AI provenance in context** — every AI output now shows which provider and data
  region produced it ("Processed by Mistral · EU"): on Scout replies, the email
  summary strip, and the document extraction panel. A reusable region badge (EU =
  green, elsewhere = amber) now also backs the AI-settings and Data & Privacy pages.
- A **security activity log** (Settings → Security → "Sign-in & security activity") —
  a per-user, paginated record of sign-ins, two-factor changes, password changes,
  data exports, and account-deletion requests. It's included in your data export and
  is automatically pruned after 12 months.
- **Documents in folders** — file a document into one or more custom folders (from the document page) and filter the Documents page by folder. Folders now organize documents as well as emails.
- **Rename a custom folder** — and the change renames the real folder/label on every connected account (Gmail, Zoho, and Microsoft), not just inside Campbooks.
- **Folder pages** — open any custom folder (from its settings) to see everything filed in it: its emails and documents together on one page. The inbox folder list also shows a document count per folder.
- A **Data & Privacy** settings page (Settings → Data & Privacy) — a privacy-framed
  overview of how your workspace's data is handled: a global **AI processing**
  switch that pauses all AI (Scout, triage, tagging, summaries, embeddings, and
  document analysis) in one click; a read-only summary of which provider and data
  region handles each AI task; the third-party services connected to the workspace;
  and quick links to export or delete your data. Existing AI-derived data (summaries,
  tags) is kept when AI is paused.
- Configurable folder icons — the inbox folder bar now renders an icon on every folder chip, and custom folders can be given an icon from a picker when created.
- A collapsible folder pane in the desktop inbox — system and custom folders as a vertical list with icons, counts, and a collapse-to-icons toggle; a custom folder's icon can be changed, or the folder deleted, from the pane. The horizontal chip bar still serves folders on mobile.
- Nested folders — custom folders can be organized into a tree in the folder pane (collapsible per branch); move a folder under another, or back to top level, from its settings. Each folder still maps to a flat provider folder by its name.
- Official production container images, published to the GitHub Container
  Registry (`ghcr.io/notacamp/campbooks`) when a release is published. Multi-arch
  (`linux/amd64` + `linux/arm64`) and tagged by semantic version (`1.2.3`, `1.2`)
  plus `latest` for the newest stable release, so self-hosters can pull a prebuilt
  image — on x86 or ARM — instead of building from source. Images are
  tagged by semantic version (`1.2.3`, `1.2`) plus `latest` for the newest stable
  release, so self-hosters can pull a prebuilt image instead of building from
  source. The full test suite re-runs as a gate before any image is pushed.
- A **Select mode** for the inbox — a toolbar toggle that turns the thread list
  into a batch organizer: persistent checkboxes on every row *and* every date /
  Priority section divider (so multi-select works on touch, not just on hover),
  tap-a-row-to-select, a select-all-per-section checkbox with an indeterminate
  state when only some of a section's threads are picked, and the docked
  bulk-action bar (archive, tag, snooze, move, delete, …). Toggle off or press
  Esc to exit.
- A machine-readable [OpenAPI 3 specification](openapi.yaml) for the public REST
  API, plus an expanded reference ([docs/api.md](docs/api.md)) with per-resource
  response examples, Python/JavaScript samples, and a complete error-code table.
  Settings → API access now links to the documentation (the URL is configurable
  via `API_DOCS_URL`).

- Real-time inbox sync: email CRUD changes (new mail, archive, snooze, trash, pin,
  tag, read/unread, folder moves, sender blocks) now reflect live across every open
  inbox — across browser tabs, devices, and teammates sharing a mailbox — without a
  manual reload. Uses the app's existing Solid Cable + Turbo Stream infrastructure;
  broadcasts are targeted and permission-scoped, so users only see changes on the
  mailboxes they can read.


### Fixed

- Closing the event editor now works when it's opened as a full page (a direct
  link, an event opened in a new tab, or a bookmarked URL): the "X" and "Cancel"
  controls take you back to the calendar instead of doing nothing. Inside the
  calendar's pop-up they still just close the dialog as before.
- The inbox view switcher (Default / List / Board) is now reachable at all
  viewport sizes in List mode — it was previously hidden on narrower widths
  because the list pane's responsive `hidden` class wasn't overridden. Also
  floats to the left of the header on short desktop viewports where the
  bottom-right email drawer would otherwise bury it. [#54]
- Publishing a domain event (`Events.publish`) no longer aborts on an internal
  `NameError` from a leftover metrics call — events publish cleanly again and
  event-triggered workflows fire reliably.
- Document analysis now reliably extracts structured fields, so documents in the
  review queue stop showing up empty. Several problems combined to leave most
  reviewed documents with no extracted data: the Anthropic (Claude) adapter sent
  an invalid API-version header and failed every call; PDFs analysed through
  OpenAI were flattened to just their first page (losing pages 2+ and misreading
  amounts); Word (`.docx`) files were classified from their filename alone; and the
  review card hid data the AI had stored under non-standard keys (e.g. a boarding
  pass's flight number, gate, and seat). Document analysis can now run on Claude —
  which reads full, multi-page PDFs natively — `.docx` text is extracted and
  analysed, and the review/Skim card always surfaces whatever was extracted. Two
  maintenance tasks apply this to existing data:
  `rails ai:route_documents_to_anthropic` points a workspace's document analysis at
  Claude, and `rails documents:reprocess_blank` re-analyses queue documents that
  previously came back empty (both support `DRY_RUN`/`WORKSPACE_ID`/`LIMIT`).
- Switching an AI role to a new provider in Settings no longer leaves a model from
  the old provider attached (which the new provider would reject) — the model
  resets to the new provider's default unless the chosen one is valid for it.
- The `emails:write` API scope description shown in Settings → API access no
  longer overstates what it grants — it marks emails read/unread (it does not
  archive, snooze, or tag).
- Drag-and-drop and tap-to-move no longer offer Sent or Drafts as destinations (moving received mail into outbound/compose folders made no sense).
- The Zoho data-center region (`ZOHO_REGION`, default `eu`) is now honored across
  every Zoho integration — mailbox sync, OAuth sign-in/connect, calendar, and
  WorkDrive — instead of being hardcoded to the EU data center. Self-hosters whose
  Zoho account lives in another region (US, IN, AU, JP, CA, CN, SA) can point at
  their own data center; the default is unchanged.
  
### Security

- Deleting your account now **revokes the external OAuth grants** it held, not just the
  local rows: connected Google/Zoho mailboxes and calendars (already), and now your
  **Google Drive** grant too. Notion has no token-revoke API, so its access is removed on
  our side and the delete-confirmation page tells you to remove the integration in Notion
  to fully revoke it.
- AI features now only process your data through a provider your workspace has
  explicitly configured (or, on a self-hosted install, the operator's own API
  keys). Two fallbacks that could route content to a shared platform provider you
  never chose are now closed on the hosted product: the text-AI surfaces (Scout
  chat, triage, classification, replies, contact analysis) no longer fall back to
  a shared Anthropic key, and embeddings (semantic search, tag suggestions) no
  longer fall back to a shared OpenAI/Gemini key. When no provider is configured a
  feature now does nothing rather than silently using one. Self-hosted behavior is
  unchanged — those keys are the operator's own and stay on their infrastructure.
  Part of the data-governance work giving users control over which AI sees their
  data.

### Changed

- ⚠️ Several features that aren't production-ready yet now ship **disabled by
  default** and are opt-in via environment flags (all default off, in both cloud
  and self-hosted builds). Set the matching var to `1` to re-enable:
  - **Workflow engine** (`ENABLE_WORKFLOWS`) — the builder UI, navigation/Cmd+K
    entries, controllers, public webhook ingress, public API, and the automatic
    email/event triggers are all gated; when off the UI/API return 404 and no
    workflow fires.
  - **Inbox "Board" (kanban) layout** (`ENABLE_EMAIL_BOARD`) — the inbox view
    switcher offers only Default and List; the board route returns 404.
  - **Microsoft 365** (`ENABLE_MICROSOFT`) — every Microsoft surface, now
    including "Sign in with Microsoft" (previously always shown), is hidden. This
    supersedes the old `ENABLE_MICROSOFT_MAILBOX` flag, which is still honored for
    backward compatibility.
- Features specific to the managed Not A Camp cloud service (e.g. the in-app
  support chat, analytics, and observability) now live in a separate private
  package, installed only through an optional `:cloud` Bundler group that is
  **excluded by default**. Self-hosting is unaffected: `bundle install` skips the
  group, never contacts the private repository, and needs no extra credentials —
  the open-source core stays free of managed-service code. (Prometheus `/metrics`
  observability, briefly added here, moved into that package and is no longer part
  of the open-source build.)
- Inbox thread-list Turbo responses (archive, unarchive, pin, snooze) no longer
  duplicate the live broadcast — the per-user cable broadcast owns the row
  insertion/removal, so the acting tab's request response is simpler and the
  two can't race.


## [0.1.0] - 2026-06-25

### Added

- **Auto-ingest document links from emails** — when an email body contains
  direct links to downloadable files (PDFs, Office documents, spreadsheets,
  CSVs), Campbooks now downloads them safely and creates Documents that flow
  through the same AI analysis pipeline as regular attachments. Fetching is
  SSRF-guarded (UrlGuard), content-type-verified, size-capped (25 MB),
  and per-link failures are isolated so one broken link never blocks the
  rest. Cloud-share links (Google Drive, Dropbox, WeTransfer) are deliberately
  skipped in this first iteration. \[#56\]

- Initial public, source-available release of Campbooks.

[Unreleased]: https://github.com/notacamp/campbooks/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/notacamp/campbooks/releases/tag/v0.1.0
