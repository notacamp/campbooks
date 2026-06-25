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

- **AI provenance in context** — every AI output now shows which provider and data
  region produced it ("Processed by Mistral · EU"): on Scout replies, the email
  summary strip, and the document extraction panel. A reusable region badge (EU =
  green, elsewhere = amber) now also backs the AI-settings and Data & Privacy pages.
- A **security activity log** (Settings → Security → "Sign-in & security activity") —
  a per-user, paginated record of sign-ins, two-factor changes, password changes,
  data exports, and account-deletion requests. It's included in your data export and
  is automatically pruned after 12 months.
- **Documents in folders** — file a document into one or more custom folders (from the document page) and filter the Documents page by folder. Folders now organize documents as well as emails.
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
- Prometheus metrics at an internal `/metrics` endpoint ([yabeda](https://github.com/yabeda-rb/yabeda)):
  HTTP request rate / error rate / latency (RED), background-job execution counts
  and duration, and a domain-action counter sourced from the Events bus. Meant to
  be scraped over a private network and visualized in Grafana. Multi-process safe
  via the Prometheus client's `DirectFileStore` (`PROMETHEUS_MULTIPROC_DIR`), with
  the Solid Queue worker exposing its own metrics server on `:9394`. See
  [docs/observability.md](docs/observability.md).
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

### Fixed

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

## [0.1.0] - 2026-06-25

### Added

- Initial public, source-available release of Campbooks.

[Unreleased]: https://github.com/notacamp/campbooks/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/notacamp/campbooks/releases/tag/v0.1.0
