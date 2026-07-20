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

### Security

- Patched `loofah` (2.25.2) and `rails-html-sanitizer` (1.7.1) — upstream
  sanitizer-bypass advisories (`javascript:` URI and SVG `href` filter
  evasion). These gems sanitize untrusted email HTML in Campbooks.

### Fixed

- Gmail attachments are ingested again: every Gmail message synced with
  "no attachments", so files were never downloaded and never became Documents.
  Gmail's metadata fetch carries no MIME part tree — attachment presence is now
  derived from the message's Content-Type (`multipart/mixed`), and the ingest
  path accepts boolean provider flags so a type drift can't silently zero the
  flag again. A new maintenance task
  (`bin/rails emails:backfill_gmail_attachments`, dry-run by default) repairs
  already-synced Gmail mail by re-checking the provider and ingesting the
  missed attachments.

## [0.28.2] - 2026-07-14

### Fixed

- Reply All on a Zoho mailbox no longer adds you as a recipient of your own
  reply. Zoho returns message metadata HTML-escaped (`&lt;you@example.com&gt;`),
  which slipped past the composer's own-address exclusion — addresses and
  subjects are now decoded when mail is synced, the composer decodes
  already-stored messages, and a one-time background sweep repairs existing
  rows (fixes stray `&amp;`/`&lt;` showing in senders and subjects too, no
  manual steps).

## [0.28.1] - 2026-07-14

### Fixed

- Events created inside Campbooks before v0.28.0 — including AI-generated
  ones from emails — now offer the guests editor. They were stored without
  the "you organize this" flag, and sync's change-detection skip meant the
  provider could never correct it; a one-time backfill repairs existing
  events (no manual steps), and provider write responses now keep the
  organizer flag and guest list up to date going forward.

## [0.28.0] - 2026-07-14

### Added

- You can now invite guests to calendar events. The event form gained a
  guests field with contact autocomplete — invitees get a regular email
  invitation from your calendar provider, and their yes/maybe/no responses
  sync back and show live next to each guest. Events you were invited to
  show the full guest list read-only, and your own response is a one-tap
  Yes / Maybe / No control.

### Changed

- The event create/edit form was redesigned: a promoted title field,
  compact date and time chips that keep the event duration when you move
  the start, an all-day switch that hides the times, icon-led rows for
  guests/location/description, and the calendar + event-type pickers
  tucked into a compact settings row. Events on calendars you can't edit
  get a cleaner read-only view with the guest list and a join-video link.

### Fixed

- Calendar sync no longer corrupts an event's guest list when writing to the
  provider: editing a synced event with guests used to push empty attendee
  entries to Google (breaking the update), updates and RSVP changes reset the
  other guests' responses to "pending", and Zoho writes dropped attendees
  entirely. Guest lists are now only written back for events you organize.

## [0.27.0] - 2026-07-14

### Added

- Files can now be browsed as a grid of thumbnail tiles: a new List/Grid
  switcher on the Files page (remembered per browser) shows each file's first
  page — PDFs and images render a real preview, other types keep their icon —
  with a sort menu replacing the table headers in grid mode. Thumbnails are
  generated on the worker as documents arrive; existing documents are swept
  by a one-shot background backfill on upgrade (no manual steps). (#307)

## [0.26.2] - 2026-07-13

### Fixed

- Marking mail read/unread in Campbooks now reaches the Zoho mailbox — Zoho
  rejected the previous API mode string on every call, so read state silently
  never synced back. (#296)
- Zoho Calendar events sync into Campbooks again: event pulls are issued in
  ≤31-day slices (Zoho caps the requested range), where previously every pull
  was rejected and no Zoho events ever appeared. The minute-cadence poll stays
  a single request covering the next month. (#297)
- Creating, updating and deleting events on a Zoho calendar now works: the
  event payload no longer contains JSON nulls (which Zoho answers with a 500),
  and the concurrency etag is sent as a request header as Zoho requires —
  previously updates and deletes were rejected outright. (#298)
- Embedding ingest now survives provider outages: transient errors (rate
  limits, timeouts) retry with backoff instead of permanently skipping the
  chunk, and the re-embed sweep fails visibly instead of halting silently when
  the provider becomes unavailable mid-run. New managed workspaces default to
  the EU Mistral embedding model so semantic search works out of the box. (#295)
- The System Health call log no longer stores full AI request/response bodies
  for successful calls (they carried email-derived text and vector arrays) and
  prunes successful AI rows after 7 days — the log had grown to several
  gigabytes in about a week. (#299)
- Managed document analysis works again: Mistral retired the `pixtral-large`
  model id (the API now rejects it), so the managed vision default moves to the
  multimodal `mistral-medium-latest`. Embedding requests are also packed to a
  per-request character budget — Mistral rejects oversized batches outright,
  which previously stalled the re-embed sweep. (#295)

## [0.26.1] - 2026-07-13

### Fixed

- Clicking a contact on the Contacts page no longer shows a "Content missing"
  error — the row now opens the contact's page as a full navigation. (#288)
- The parked-draft "Draft saved" pill no longer appears on the compose page,
  where it sat on top of Scout's chat input and blocked typing. It still
  surfaces on every other page. (#289)
- Scout's compose assistant now drafts the email body immediately when you
  describe what to say, instead of stopping after recipients/subject and never
  writing the draft. It also no longer guesses recipient addresses — it only
  uses addresses you typed or known contacts, and asks otherwise. (#290)
- The "Templates" button in the composer footer was missing its translations
  in all languages and rendered a raw translation-missing string. (#291)
- Undoing an archive now puts the conversation straight back into the list you
  were viewing (folder or search included) — previously it only reappeared on
  the unfiltered inbox or after a manual reload. (#294)

## [0.26.0] - 2026-07-12

### Removed

- ⚠️ **The legacy extracted-value columns were dropped from `documents`.** Since
  v0.24.0 every extracted field lives in the `documents.metadata` JSONB store;
  this release removes the 23 dead columns (`vendor_name`, `amount_cents`,
  `document_date`, `expense_category`, …) and their indexes. Self-hosters
  querying those columns directly must switch to `metadata->>'field'`. Rolling
  back past this release requires a database backup taken beforehand — the
  upgrade itself migrates all values safely and automatically.

## [0.25.0] - 2026-07-12

### Added

- **Sort and filter your documents by their extracted fields.** Filter the Files
  page to a single document type and its fields become part of the page: the table
  swaps in that type's most useful fields as sortable columns (click a header to
  sort, click again to flip direction), the filter panel grows a "fields" section
  with the right control per field — text contains, amount ≥/≤ (in euros), date
  ranges, category pickers, yes/no — and active field filters show as removable
  chips. All of it is driven by the type's field schema, so adding a field to a
  custom type (Settings → Inbox → Document types) makes it sortable and filterable
  with no code changes. Sorting is also available from the filter panel on mobile,
  and exports respect the active field filters.

### Changed

- Money field labels dropped the "(cents)" suffix — list columns and filters show
  formatted amounts; the document edit form keeps a cents hint under the input.
- Built-in document types lead with their most scannable fields (name, amount,
  dates) in both the Files columns and the document edit form.

## [0.24.0] - 2026-07-12

### Changed

- ⚠️ **Every document type is now schema-defined.** The five built-in types
  (expense invoice, revenue invoice, receipt, bank statement, other) carry the same
  kind of field schema custom types always had — one uniform, editable definition
  per type (Settings → Inbox → Document types) that drives extraction, the document
  edit form, and review. New fields added to a type's schema flow through the whole
  app with no code changes.
- ⚠️ **Extracted document values now live in one place.** Values previously split
  between dedicated database columns and the document's metadata were merged into
  the metadata store (an idempotent migration runs automatically on upgrade; the
  old columns are retained for now and will be dropped in an upcoming release —
  self-hosters querying `documents` columns directly should switch to
  `metadata->>'field'`). The public API request/response shapes are unchanged.
- **Documents in non-EUR currencies now display in their own currency.** Amounts
  were previously always formatted as EUR regardless of the document's currency.

### Fixed

- **Document classification no longer crosses workspaces.** A document could be
  linked to another workspace's document type of the same name; classification is
  now always scoped to the document's own workspace, and documents pick up their
  workspace's type on creation.

## [0.23.2] - 2026-07-12

### Fixed

- **The v0.23.0 embedding migration no longer needs a large `/dev/shm`.** pgvector's
  parallel HNSW index build allocates a ~64MB shared-memory segment, which fails with
  `PG::DiskFull` inside containers running Docker's default 64MB `/dev/shm` — even
  though the new index columns are empty. The migration now builds those indexes
  single-process, and the self-host `docker-compose.yml` gives Postgres
  `shm_size: 512mb` for future index work.

## [0.23.1] - 2026-07-12

### Changed

- **The "Extracted data" panel in document review (Skim) stays open as you move
  between cards.** Expand it once and it stays expanded on the next document, so you
  can keep the AI-extracted fields in view without re-opening the panel each time.

### Fixed

- **Document review (Skim) keyboard shortcuts work again when a PDF is on screen.**
  The arrow keys (skip / go back) and the A / C / E / R / J / O / Space shortcuts
  stopped responding whenever a PDF preview loaded — the browser's inline PDF viewer
  was quietly capturing the keyboard. Skim now keeps keyboard focus on the review
  card, so you can move through the queue entirely from the keyboard again.

## [0.23.0] - 2026-07-12

### Added

- **Choose your semantic-search embedding model per workspace.** Settings → AI
  gains an "Embedding Model" section where each workspace picks the model that
  powers email/document search and tag classification: OpenAI
  `text-embedding-3-small` (the previous fixed default) or `text-embedding-3-large`,
  Google `gemini-embedding-001`, or Mistral `mistral-embed` — the Mistral option
  keeps embedding in the EU, so workspaces with the EU data-residency policy get
  semantic search for the first time. Switching re-embeds the workspace's search
  index safely in the background with progress shown in Settings; search stays
  correct (temporarily partial) while it runs. See `docs/embeddings.md`.

## [0.22.4] - 2026-07-11

### Changed

- **Non-document attachments no longer cost an AI vision call.** Document processing
  now splits by file type: real documents (PDF, image, Office) go through AI/OCR
  analysis as before, while non-documents — calendar invites, raw emails, archives,
  HTML — are stored deterministically and marked *skipped* instead of being sent to
  the model just to be labelled "other". Cheaper and faster, and it keeps those files
  out of the review queue (as before) without the round-trip. They can still be
  analysed on demand from the Files area.

## [0.22.3] - 2026-07-10

### Fixed

- **Search embedding no longer fails on image-only / empty-body emails.** These
  hit a chunker path that returned a malformed (doubly-nested) chunk and raised a
  `TypeError`, so the message never got indexed for search. They now embed
  correctly.
- **AI extraction backfills no longer overwhelm the job queue.** Reminder and task
  extraction now pace themselves (≤2 in flight) and retry patiently on provider
  rate limits (HTTP 429) instead of failing outright — matching contact analysis —
  so re-processing a large mailbox's history can't spike into a 429 storm or a
  pile-up of failed jobs.
- **Background search-embedding runs at a lower priority** so a bulk re-embed of a
  mailbox can no longer delay user-triggered work — e.g. running an inbox rule on
  existing mail, which previously queued behind the whole backfill.
- **Search-record finalization tolerates a concurrency race** (duplicate-key
  retry) when a searchable record's last chunks finish embedding simultaneously.

## [0.22.2] - 2026-07-10

### Fixed

- **Files: opening a file works again.** Clicking a file (or an internal document
  or filed email) in the Files list showed a "Content missing" placeholder instead
  of the document — the list lives inside the frame that live-updates search and
  filters, and since v0.21.0 the row links navigated that frame instead of the
  page. Row links (including the row menu's Open and Download) now open the full
  page, on desktop and mobile.

## [0.22.1] - 2026-07-10

### Fixed

- **Files: uploading a file now refreshes the page.** After an upload the panel
  stayed open and the new file didn't show up — and on an empty Files page (or an
  empty folder) nothing appeared to happen at all. The page render that follows an
  upload was matching the infinite-scroll pagination stream instead of a full
  refresh. Uploading now reloads the view: the new file appears at the top of the
  list, the upload panel closes, and a confirmation toast is shown.

## [0.22.0] - 2026-07-10

### Added

- **Reminders: keyboard navigation.** Move a cursor through the reminders list with **↑↓** or **j / k**, then act on the focused reminder without the mouse — **⏎** adds it to the calendar, **s** snoozes, **d** dismisses. A key legend sits above the list (desktop) and the shortcuts are also listed in the global **?** dialog. Each key fires the row's own button, so the toast, undo, and row removal are identical to a click.

### Fixed

- **Inbox search now shows it's working.** A free-text search runs a semantic
  (embedding) lookup that can take a second or two, but the only cue was a tiny
  corner spinner and a faint dim of the old results — so the pane looked frozen.
  Running a search now shows a progress bar under the search field and skeleton
  placeholder rows in the results area the moment the request fires, so the wait
  is legible instead of dead air.
- **Clearing the search box takes you back to your inbox.** Emptying the query
  (when no filters are still active) now returns you to the real inbox — grouped
  list, layout switcher and all — instead of leaving you stranded on a bare
  results list at the `/search` URL.

## [0.21.2] - 2026-07-10

### Fixed

- **The composer's "Draft saved" confirmation no longer lingers forever.** After an autosave the composer showed a "Draft saved" label that never went away — in the floating compose dock it read like a toast that refused to dismiss. It now fades out a couple of seconds after saving, while the transient "Saving…" state still shows while a save is in flight.

## [0.21.1] - 2026-07-10

### Fixed

- **Emails sent from Campbooks never appeared in the Sent folder.** The locally recorded copy was filed under a `"sent"` placeholder id that the Sent folder view (which filters on the provider's real folder ids) could never match — and the sync kept the placeholder forever. Sends now record under the account's real Sent folder id (visible immediately), and the next scan heals any existing placeholder rows.
- **One un-parseable Zoho message no longer stalls a mailbox's sync.** Zoho truncates message summaries and can cut an emoji in half, producing invalid JSON that poisoned the whole page — freezing that folder and every folder after it in the scan (deep pages permanently blocked the 15-minute full reconcile). Zoho responses now scrub lone UTF-16 surrogate escapes on parse failure, and a folder that still errors is skipped and recorded on the scan log instead of aborting the remaining folders.

## [0.21.0] - 2026-07-10

### Added

- **Files: search modifiers.** The Files search box now understands Gmail-style modifiers alongside free text — `type:receipt`, `category:accounting`, `vendor:EDP`, `number:FT2026/448`, `amount>100`, `amount<500`, `after:2026-01` / `before:2026-06-30`, `is:starred|pending|approved|rejected|failed|processing`, `source:email|upload|notion|sent`, `expense:travel`, and `in:<folder>` — with a typeahead that suggests modifiers and their values as you type (mirroring the inbox search).
- **Files: full filter panel.** The old five-field strip is replaced by a filter panel on the search bar covering document type (multi-select), category, review status, processing status, source, starred, document-date range, amount range, vendor/entity, reference number, expense category, and folder. Active filters show as removable chips above the results, and the results update live without a page reload.
- **Files: filtered bulk actions.** "Reanalyze" and "Export" now honor everything the list is filtered by — panel filters and search modifiers alike — so the ZIP matches exactly what's on screen.

### Fixed

- **Document exports produced no ZIP.** The export job still filtered on a `status` column that was split into `ai_status`/`review_status`, so every export errored; it now exports analyzed, non-rejected documents. Existing export records with legacy filters remain readable.

## [0.20.0] - 2026-07-10

### Added

- **Inbox rules:** workspace-scoped, user-defined deterministic rules that evaluate against every newly ingested email and can be run retroactively. Each rule matches criteria (from, to, subject, body, category, email account, has attachment) and applies actions: tag, archive, mark as read, move to a custom folder. Runs are undoable when the match set is <= 25,000 emails. Managed via **Settings → Inbox → Rules** with live match-count preview, run-progress polling, and per-run undo.

### Fixed

- **Inbox groups: rules added to a group are now saved.** The Groups builder submits its rules as indexed form fields, which the server parsed as a hash rather than a list — so every sender/organization/document-type/query rule was silently dropped, and a rules-only group saved nothing at all. Rules now persist on create and update.
- **Inbox groups: a stray blank-named group no longer shows an un-editable row.** A group heading is always non-blank, but a legacy blank-named row could still render an Edit/Ungroup link with no group name, so clicking Edit appeared to do nothing. Blank-named rows are now filtered out of the Groups panel and the inbox.

## [0.19.9] - 2026-07-10

### Changed

- **Accounting: match confidence is now governed by entity agreement, not amount proximity.** A near-amount invoice from a clearly different organization can no longer be suggested (a close-amount pairing needs positive name/entity evidence; without it, it's discarded with the reason recorded). Exact-amount matches without entity evidence cap at "Possible"; strong confidence requires exact amount *and* the same party. The AI now also sees each candidate invoice's own description so it can judge what the invoice is for against who actually received the payment.

## [0.19.8] - 2026-07-10

### Fixed

- **Accounting: bank-statement balance/summary lines no longer appear as phantom 0.00 transactions.** Opening/closing/available-balance rows ("SALDO INICIAL"…) are excluded in the parsing prompt and any zero-amount row is dropped before insert — a real movement is never zero.

## [0.19.7] - 2026-07-10

### Fixed

- **Accounting: multipage PDF statements only sent their cover page to the AI.** The page counter used `identify "file.pdf[0]"` with `%n`, which reports the size of the *selected* list — always 1 — so only page one was rasterized and statements whose transactions live on later pages parsed to zero. Pages are now counted from the PDF's actual frame list.

## [0.19.6] - 2026-07-10

### Fixed

- **Accounting: multipage PDF statements rendered as junk thumbnails on OpenAI/Mistral providers.** Page selection used MiniMagick's `Image#page` — which sets canvas geometry, not the page — so multi-page statements (e.g. Millennium combined extracts) reached the model as unreadable images and parsed to zero transactions. Pages are now rendered through ImageMagick's convert tool (`-density 150 input.pdf[N]`), suspiciously small renders are treated as failures, and the shared document-analysis rasterizer got the same fix (multi-page invoices were affected too).

## [0.19.5] - 2026-07-10

### Fixed

- **Accounting: AI match suggestions are now grounded in the transaction amount.** The AI disambiguator could suggest an invoice whose amount doesn't fit the payment — with high claimed confidence and an invented justification. Model confidence is now bounded by evidence: exact amounts keep the model's score, near amounts (±2%) are capped below "strong", anything else is discarded (split payments whose invoices sum to the transaction are kept). Near-duplicate documents (a receipt and its invoice for the same purchase) collapse into a single suggestion. Every AI disambiguation run is also audited to the database (`bank_transactions.ai_match_debug`: candidates sent, raw model claims, and each grounding decision) for debugging, and the matching prompt now states hard evidence rules — amount is primary, no invented company relationships, empty is a good answer — with a strict confidence rubric.

## [0.19.4] - 2026-07-09

### Changed

- **Accounting: documents preview in place.** Clicking a matched/suggested invoice chip, a search candidate in the resolve panel, or the new "View statement" chip opens the document in a slide-over preview (bottom sheet on mobile) without leaving the workbench — the file renders inline, with an "open full page" escape hatch. Ctrl/Cmd+click still opens the full document page directly.

## [0.19.3] - 2026-07-09

### Changed

- **Accounting: match chips open the document.** The matched/suggested invoice chip in the workbench rows, cards, and resolve panel now links to the document (new tab), so you can inspect the actual invoice before confirming a match.

## [0.19.2] - 2026-07-09

### Fixed

- **Settings → General wiped the workspace context on save and never persisted the VAT number.** The form scoped its fields under `user[...]` while the controller read top-level params, so saving the page cleared `workspace_context` and dropped `company_nif`. The form now submits the exact param names the controller reads, both writes are guarded so a request that omits a field never clears it, and specs pin the form's input names against the controller contract. `Workspace#company_nif` also falls back to the onboarding-collected company tax id, so the accounting NIF check works without re-entering the same number.

## [0.19.1] - 2026-07-09

### Fixed

- **Accounting: PDF statement parsing on OpenAI/Mistral providers extracted zero transactions.** Page rasterization passed ImageMagick's bracket page syntax to `MiniMagick::Image.open`, which MiniMagick 5 rejects — every page silently failed and the AI received an imageless prompt. Pages are now selected via `#page` after opening (the same mechanism the document analyzer uses). A statement that yields no readable pages or no transactions now fails loudly with a clear message instead of completing as "ready" with an empty table.

## [0.19.0] - 2026-07-09

### Added

- **Accounting module — bank reconciliation (`ENABLE_ACCOUNTING=1` + `:accounting` entitlement).** A new top-level module for small-business bookkeeping: pick a bank statement Campbooks already filed from your email (or upload one) and every debit is matched to its expense invoice, every credit to its sales invoice.
  - **Statement parsing**: CSV extracts parse deterministically (delimiter + encoding detection incl. UTF-8 BOM and Latin-1, multilingual column aliases in pt/en/es/fr, three amount layouts, EU/US decimal formats, balance integrity + sign-convention checks). PDF statements parse via AI (`Ai::BankStatementParser` — native multi-page PDF on Anthropic/Gemini, per-page rasterization on OpenAI/Mistral, chunked for long statements).
  - **Matching engine**: `Reconciliations::Matcher` scores candidates by amount exactness, date proximity and name similarity, optionally disambiguating with the workspace's text AI; strong matches are auto-suggested with plain-language reasons and live Turbo Stream progress.
  - **Workbench**: confirm/reject suggestions inline, bulk "Confirm all", manually attach any document, exclude transactions with a reason (bank fee, salary, transfer, tax), or **request the missing invoice** — a pre-filled email opens in the composer (standard or corrected-NIF variant); nothing is ever sent automatically.
  - **VAT number check (optional)**: set your company NIF in Settings → General and expense invoices missing it (or carrying a different one) are flagged inline, counted in the summary bar, and reported in the export.
  - **Accountant export**: a background job builds a zip with the original statement, matched invoices renamed `YYYY-MM-DD ±amount vendor invoice-no` in `debits/`/`credits/` folders, and an `index.csv` listing every transaction, its resolution and NIF status.
  - Ships with notifications (ready for review / parse failed / zip ready), a demo-workspace seed reconciliation, and full four-locale i18n.

## [0.18.1] - 2026-07-07

### Fixed

- **A mailbox backfill no longer floods your feed with stale suggestions from old email.** When a full resync or a newly-connected account ingested months- or years-old mail, the AI task/reminder extractors ran on every message and — because a task suggestion had no recency check — could mint hundreds of suggestions at once, many dated to the wrong year (an invoice "due in 2024", a relative date rolled forward into this year). Suggestions are now staged only while still temporally live: a **future-dated** action is always kept; an **overdue or undated** one only when its source email is still recent. A genuinely forgotten recent action still surfaces — framed as **"You may have missed this"** on the feed — while a long-dead ask from a backfill is dropped. Drafting a calendar event from an old email now anchors to the email's own date instead of always defaulting to tomorrow.

## [0.18.0] - 2026-07-06

### Fixed

- **Campbooks' own digest emails are no longer mined by the AI pipeline.** Digests
  are delivered to your mailbox, so the email scanner re-ingests them; previously the
  reminder / task / contact extractors ran on them, so a digest that *lists* your
  reminders could spawn duplicate ones. Campbooks-generated mail is now recognised at
  ingest — via an `X-Campbooks-Kind` header we stamp on the way out, with a
  sender-address fallback for providers (e.g. Zoho) that strip custom headers — and
  skips all AI analysis while staying fully readable in the inbox. Digests are
  marked with a **Digest** badge so you know they're ours to read.
- An in-progress inbox no longer claims "All caught up" when the user navigates away from the first-sync stage while a scan is still running. Home now shows an honest "Scout is reading your inbox" state.

### Added

- **First-run walkthrough rebuilt as an explanation-first slideshow.** What Campbooks is (with rotating Scout statements), then Inbox, Calendar, Tasks (feature-gated), Documents, and a "much more" close with docs link and connect CTA. Segmented progress bars with 0.4s fill animation, module label + slide counter, per-slide vignette animations (chips pop, group counter ticks, event and reminder buttons confirm, task ticks, document fields fade in), keyboard navigation, and click-to-jump segments. New registrations land on home where it auto-opens.
- **Tag filing is now do-and-tell: Scout files the email and tells you in the feed — one tap undoes it.** (Previously the feed asked before filing.)
- The first-sync wait screen now asks what you mostly deal with (persona setup applies mid-scan via a compact chip picker; a Turbo Stream swaps the card for a confirmation once submitted).
- Skipping the first-sync stage via the escape hatch now actually skips it — a POST sets a session flag so home does not re-trap you in the stage on subsequent visits.
- **Scheduling emails now show the drafted event inline — one tap adds it to your calendar (Edit still opens the full form).** When Scout detects a time proposal in an email (e.g. "does 3pm work?"), a bordered event block appears below the thread with the extracted title and time range. Tap "Add to calendar" to confirm it in one step; tap "Edit" to open the prefilled calendar form. Scout drafts — you decide.

- **MCP: `archive_emails` tool to clear inbox noise at scale.** Agents can now
  archive every email matching a filter — a whole tag (e.g. `Notifications`), a
  date range, a sender, or an AI priority — instead of being capped at
  `update_emails`' 100-id batches. Call it with `preview: true` first to see how
  many messages match without touching anything, then again to archive them
  (reversible via `update_emails(action: unarchive)`). Requires the `emails:write`
  scope. The bulk-archive tag filter now matches case-insensitively, so a
  capitalized group tag (`Notifications`, `Social`, …) matches a lower-cased value.
- **MCP: tag-taxonomy management tools — `update_tag`, `merge_tags`, `delete_tag`.**
  Agents can now rename/recolour/regroup a tag, fold duplicate tags into one
  (`merge_tags` re-tags every email and task from the sources to the target, then
  deletes the sources — the safe way to collapse `notifications` into
  `Notifications`), and delete a tag (guarded: refuses a tag that still labels
  emails unless `force: true`, and never deletes the system `security_flagged`
  tag). Tags resolve by id or name. All require the `tags:write` scope.

### Changed

- **Removing an email account now actually removes it — and the button works.** In
  Inbox settings → Accounts, the account action is now labelled **Remove** and does
  what it says: after a confirmation, it deletes the mailbox connection and everything
  synced from it — emails, threads, folders, scan history, scheduled sends — and revokes
  Campbooks' access at the provider (unless a still-connected calendar shares the same
  sign-in). Documents you've filed and contacts you've saved are **kept**; they're just
  detached from the removed mailbox. The button previously did nothing when clicked;
  the teardown now runs in the background so even a large mailbox can't stall it.
- **Default inbox noise buckets now collapse even after you reply or when a
  sibling message is important.** The four built-in tag groups (Notifications,
  Newsletters & promos, Social, Updates) used to stay in the main inbox list
  whenever the "a human cares about this thread" guards fired — including when
  you'd once replied to the thread, or another message in it was categorized
  `important` — which leaked automated mail back into the inbox. Those buckets now
  honor only the explicit signals: pinning a thread or starring its sender still
  keeps it inline, but a stray reply or important sibling no longer does. Custom
  (user-defined) tag groups and rule-based groups are unchanged — they keep the
  full guard set.

### Fixed

- **Inbox search now shows it's working.** A free-text search runs a semantic
  (embedding) lookup that can take a second or two, but the only cue was a tiny
  corner spinner and a faint dim of the old results — so the pane looked frozen.
  Running a search now shows a progress bar under the search field and skeleton
  placeholder rows in the results area the moment the request fires, so the wait
  is legible instead of dead air.
- **Clearing the search box takes you back to your inbox.** Emptying the query
  (when no filters are still active) now returns you to the real inbox — grouped
  list, layout switcher and all — instead of leaving you stranded on a bare
  results list at the `/search` URL.

## [0.17.0] - 2026-07-06

### Added

- **Gmail-style `g` navigation shortcuts for the left nav rail.** Press `g` to arm navigation mode (a 3-second window), then a second key to jump directly to any section: `h` Home, `m` Mail, `c` Calendar, `s` Scout, `f` Files, `p` Contacts, `o` Organizations, `a` Activity, `t` Tasks, `d` Digests, `w` Workflows. While armed, tiny keycap badges appear beside each nav icon (desktop rail and mobile bottom bar) so the map is self-documenting. Escaping, clicking, or navigating disarms automatically. All nine destination keys are listed in the keyboard shortcuts dialog (`?`). Works on desktop; suppressed in the native app shell.
- **Workspace-owned tags with provider-label pointers.** Each tag can now be
  linked to a provider label (Gmail / Zoho) in each connected account via a new
  `tag_account_links` table, decoupling workspace tags from any single mailbox.
- **Label-import review flow.** When a label sync discovers new non-system
  labels, they appear in a review panel (Settings > Tags > "Review now"). Users
  choose per label: map to an existing workspace tag, keep it as its own tag, or
  ignore it. Decisions are remembered across re-syncs. Provider-side labels are
  never deleted.
- **Tag merge.** From the Tags panel, merge tag B into tag A with one click. All
  messages and account links move to the target tag; the source is deleted. A
  cross-workspace guard prevents accidental merges across tenants.
- **Group-row bulk selection.** Collapsed tag-group rows in the inbox now show a
  checkbox on hover (and permanently in select mode), wired to the selection
  toolbar. Checking a group row adds all its collapsed emails to the current
  selection; the toolbar count reflects the total emails, not the number of
  checkboxes. Every toolbar action (archive, mark read/unread, tag, move, snooze,
  delete, Send to Scout) works on groups via a shared server-side expansion that
  mirrors the same guarded scope the drill-in view uses. Select-all includes
  group rows alongside thread rows.
- **Onboarding setup templates.** New workspaces are greeted with a persona picker (Freelancer / Small business / Personal admin / Job hunt / Just exploring) as the first onboarding step. Selecting a template seeds the right tags and document types automatically and adjusts which nav modules are visible. Applying is fully idempotent and additive: existing tags and document types are never removed. A new Settings page (Settings > Setup) lets users review the active template, switch to a different one, and toggle individual module visibility at any time.
- **Group rules for inbox groups.** Groups can now match threads not only
  through tags, but also through rules: match by sender email or domain
  (`@acme.com`), by organization (every contact belonging to that org), by
  document type (threads that carry a document of that type), or via a
  structured search query (`from:`, `tag:`, `is:`, `has:`, `category:`,
  `priority:`, `after:`, `before:`). Rules-only groups (no tags required)
  are fully supported. Additive multi-group membership is preserved: the same
  thread can satisfy any number of groups simultaneously. The Groups panel
  in inbox settings gains a dynamic Rules section for adding/removing rules,
  and existing bulk-action flows (Mark all read, Archive all) work unchanged
  because they go through the same engine. Exposed in all four UI languages.

### Changed

- Label sync services (Google, Zoho) now record a pending `LabelImportDecision`
  for every user-visible label discovered during sync, in addition to the
  existing external-tag creation flow (fully backward compatible).

### Fixed

- Tag picker in the email reading pane no longer shows the same tag name multiple times when a workspace has several email accounts that each sync a provider label with the same name (e.g. "Work" from both Gmail and Zoho). `Tag.visible_for` now correctly scopes to the supplied workspace, and the picker's tag list is deduplicated by name — preferring local tags over external ones when names collide.
- Full-page composer (Desk) now uses its right-hand space with a context rail: original message preview (sandboxed iframe), an attachments drop-zone card, and Scout suggestion chips that prefill the chat input. Rail stacks below the editor on mobile.


## [0.16.0] - 2026-07-06

### Added

- **A Groups panel in the inbox settings.** Create a group, pick which tags
  belong to it with tap-to-toggle pills, and the inbox collapses their mail
  into that group's row — no more editing tags one by one. Groups show their
  member tags and how many conversations they currently collapse; renaming
  keeps everything working, and ungrouping keeps the tags. While creating, the
  group's name prefills from the first tag you add (until you type your own).

### Fixed

- **Tag groups now actually collapse on the inbox you land on.** The inbox
  landing URL carries the mapped inbox folder id, and the group logic treated
  any folder id as "browsing a folder" — so with a live provider connection the
  group rows never appeared and nothing collapsed. The inbox root now recognizes
  its own folder id; specific (non-inbox) folder views still show grouped mail
  inline.

## [0.15.0] - 2026-07-06

### Added

- **Default tag groups.** Every workspace now ships with four built-in groups —
  **Notifications**, **Newsletters & promos**, **Social**, and **Updates** — that
  are on by default. Mail the triage engine sorts into one of these buckets is
  automatically tagged, so the inbox collapses it into a tappable group row
  instead of scattering it through the list. Drilling into a group offers
  **Mark all read** and **Archive all**. Groups are ordinary tags: rename,
  recolor, regroup, or add your own tags to any group from **Settings → Tags** —
  a tag with a group name collapses in the inbox, and a message shows in every
  group its tags belong to.
- **Search your Organizations directory, and scroll it endlessly.** The
  Organizations page now has a search box that filters the directory by company
  name or email domain as you type, and the list loads more as you scroll instead
  of paging through it.

### Changed

- ⚠️ **The inbox now collapses low-priority mail by tag, not by a separate
  "smart groups" system.** A thread drops out of the main list when any of its
  messages carries a tag that belongs to a group — but it always stays in the
  list when you have replied to it, pinned it, starred the sender, or any message
  is important. Grouped mail now shows inline inside a specific folder view (the
  collapse applies to the inbox root only), and custom tag groups get the same
  "keep it if it matters" guards.
- **Tasks open on the Board view by default** (was the list), so you land on the
  status board — To do · In progress · Blocked · Done — at a glance. Switch to the
  flat list any time from the toggle, or link straight to it with `/tasks?view=list`.
- **Assigning people and tags to a task feels more modern.** The assignee and tag
  pickers — on the task form and on a task's page — are now tap-to-toggle pills
  (showing a teammate's initials or the tag's color) instead of a column of
  checkboxes, and you can set assignees right from the new-task form.
- **New senders are now profiled by AI based on who they are, not how many times
  they've emailed.** A real person or vendor is profiled from their first message
  — so they show up under their organisation in the Contacts / Organizations
  directory right away — while machine and bulk senders (notifications,
  newsletters, no-reply and auto-submitted mailboxes) are skipped as noise. This
  replaces the previous "5 emails from this address" volume threshold on the live
  ingest path. (The one-off backlog catch-up still uses the old volume floor for
  now.)

### Removed

- ⚠️ **The Smart groups settings panel** and its per-user bucket on/off
  preferences. Grouping is now configured entirely through Tags
  (Settings → Tags → give a tag a group). All four default groups are on for
  every workspace; existing categorized mail is tagged automatically on upgrade.

### Fixed

- Connected Google accounts with no Gmail mailbox or no Google Calendar (a
  sign-in-only / non-Gmail identity) no longer retry their sync every minute
  forever — the scan now detects the provider's "service not enabled" response,
  turns that account's sync off, and explains why on the accounts/calendars
  settings page and in the disconnect notification (instead of a generic
  "reconnect"). Stops the recurring 400/403 noise on the System Health dashboard.
- Zoho Calendar read failures now log the response status and body. The reads
  previously swallowed errors silently, making a live-grant 400 impossible to
  diagnose from the logs.

## [0.14.0] - 2026-07-06

### Fixed

- **Reminders from a round-trip booking now cover every leg, not just the outbound.**
  A flight or trip with a return date is read as two dated commitments, so the return
  departure gets its own reminder (same-day connecting flights still collapse into one).
  The same booking arriving as two emails — e.g. a booking confirmation and a separate
  ticket email — no longer creates duplicate reminders: the de-dupe now matches a timed
  reminder on its exact time and ignores a date the AI appends to the title.

### Changed

- **System health: always-on request/response capture.** Every external call now
  records its sanitized request and response headers and body (credentials
  redacted, bodies capped at 10 KB, binary payloads as placeholders). AI provider
  calls additionally store the model name and prompt/completion token counts in
  row metadata. App admins can inspect any row at the new call detail page
  (`/admin/system_health/calls/:id`); the workspace-facing view stays
  metadata-only. Workspace deletion now purges the workspace's call rows.
- **System health rows now attribute to the workspace through the account that
  made the call**, not just the ambient job context — so mailbox, calendar,
  Drive, and Notion traffic counts toward the right workspace even when
  triggered from a console or maintenance task.
- The inbox **"Waiting on replies"** band now shows only threads you've been
  waiting on for up to 30 days, always keeping at least three visible regardless
  of age. Anything older folds behind a **"Show older"** toggle, so a backlog of
  long-forgotten sends no longer buries the replies still worth chasing.

### Fixed

- **The integration settings pages read cleanly and are easier to leave.** The Google
  Drive page showed a raw "Connect Desc Html" placeholder where its description should
  be, and on an instance where Google Drive isn't set up, clicking **Connect** hit an
  error page. The description now renders properly, an unconfigured instance explains
  that an administrator needs to set it up (instead of offering a button that errors),
  and every integration detail page — Google Drive, Notion, Zoho Drive, Connections,
  and Calendars — now has a **Back to integrations** link. Disconnecting Google Drive
  also returns you to the integrations list rather than erroring.


## [0.13.0] - 2026-07-05

### Added

- **API: tags now report `kind`, `hidden`, and `email_count`,** and `GET /tags`
  accepts `?include_hidden=true` — so API and MCP clients can distinguish provider
  system labels from real tags and spot unused ones.
- **System health, for your workspace and the whole instance.** Campbooks now
  records every call it makes to an outside service (mail providers, calendars,
  storage, AI models, workflow webhooks, push, SMTP) with its outcome and
  duration. Workspace admins get **Settings → System health**: their
  workspace's services with error rates, hourly activity sparklines, the most
  recent error, and a filterable call log. Instance operators get the sum of
  all workspaces at `/admin/system_health`. Successes are kept 30 days, errors
  90; set `DISABLE_SYSTEM_HEALTH=1` to opt out of recording entirely. See
  `docs/system-health.md`.

### Changed

- The home feed's Scout summary now caps at three lines with a "Read more"
  toggle, instead of running the full read down the card. Keeps feed cards
  short and scannable; the whole read is one tap away.

- **Tags and provider labels are one concept.** Renaming or recoloring a
  synced tag now updates the label in the connected mailbox immediately;
  previously those edits were silently overwritten by the next background sync.
  Deleting a synced tag also removes it from the provider mailbox (best-effort,
  same as before). All tag and label assignment goes through a single endpoint
  so the behavior is consistent regardless of whether a tag is local or
  provider-synced.

### Fixed

- Tag chips on an inbox thread row now update on every add/remove. Previously
  the first change silently broke the row's chip area, so any later tag change
  on the same thread didn't show until a reload.
- **Hidden provider labels no longer leak through the API or MCP `list_tags`.**
  Gmail system statuses (INBOX, CATEGORY_*) and AI-judged low-value labels are
  excluded by default, matching what the inbox already hides — pass
  `include_hidden=true` on `GET /tags` to include them.
- **Synced provider labels that arrive without a colour now get a distinct,
  stable colour** instead of every one sharing a single gold default, so they're
  no longer indistinguishable at a glance.
- **A label sync that fails for one mailbox is now reported to error tracking,**
  not just written to the log — a silently-swallowed failure is how a mailbox's
  labels drift stale for months unnoticed.

### Removed

- **Settings → Inbox → Labels panel removed.** Provider labels are now managed
  through the unified Tags panel. The background label sync every 10 minutes
  still runs; provider labels continue to appear in the Tags panel as
  synced tags. Creating labels from within the app is no longer supported —
  create them directly in Gmail or Zoho and they will sync automatically.

## [0.12.2] - 2026-07-05

### Fixed

- **Skim's follow-up cards respond to the keyboard and the Dismiss button again.** On the
  Follow-ups ring, pressing **D** — or clicking **Dismiss** — now retires a follow-up as
  intended. Both were silently inert (the card's action id and theme were being rendered
  in a dasherized form the Skim keyboard handler didn't recognise); follow-ups was the only
  ring affected. Keep (→) still works, and a tray shortcut into the Follow-ups ring now
  lands there correctly.
- **Catching up a large contact backlog no longer trips the AI provider's rate
  limit.** Contact profiling used to fire the whole catch-up batch at once
  (up to a hundred concurrent AI calls), which rate-limited nearly every
  request and slowed the catch-up to a crawl. Analyses now run a couple at a
  time, and a rate-limited attempt is retried with backoff instead of being
  dropped until the next pass.

## [0.12.1] - 2026-07-05

### Fixed

- **Contact profiling runs again on hosted installs — and the Organizations
  directory fills itself.** Background contact analysis ran without the
  workspace context, so it never resolved the workspace's AI provider and
  silently skipped every contact: profiles stayed empty, the Organizations page
  stayed empty, and the self-heal re-enqueued the same contacts forever. The
  job now sets the workspace, a freshly analyzed contact materializes its
  organization (and membership) immediately instead of waiting for a manual
  "Sync from contacts", and an upgrade migration backfills organizations from
  already-analyzed contacts so existing installs catch up on their own. A
  provider-resolution failure is now logged loudly instead of passing silently.
- **People auto-merge is scoped to the workspace.** The duplicate-person
  auto-merge that runs after contact analysis matched people by name across
  all workspaces; two same-named people in different workspaces could have
  been merged together. It now only ever merges within one workspace.

## [0.12.0] - 2026-07-05

### Added

- **Workspace admins can manage their own workspace.** Settings → Members now
  shows each member's role and lets workspace admins promote or demote
  teammates (never themselves), and approve teammates' pending invitations
  right there — approval no longer requires the hosting operator. Whoever
  creates a workspace is its admin automatically; on upgrade, workspaces that
  had no admin get their earliest member promoted, so every workspace can
  manage itself without manual steps.
- **Being assigned a task now notifies you.** A bell notification (with the
  task title, linking to the task) fires for each newly assigned person —
  assigning yourself stays silent. `@mentioning` a teammate in a task
  discussion now works like email discussions: they're subscribed to the
  thread and notified.
- **Scheduled digests.** Build your own recurring briefings: a digest is a saved
  scope (emails matching a search query, upcoming calendar events, tasks due soon,
  reminders, recently received documents — mix and match), a schedule (daily,
  weekly, or monthly at your chosen time), an optional AI summary, and a delivery
  choice (email and/or a home-feed card). Six presets get you started — Newsletter
  roundup, Week ahead, Upcoming tasks, Invoice tracker, Client pulse, or fully
  custom — and every issue is kept, so `/digests` doubles as a browsable archive.
  With AI enabled, Scout groups the period's items into thematic sections with a
  short overview and per-item notes (every line links back to the real email,
  event, task, or document — nothing is invented, and anything the AI doesn't
  place lands in an "Everything else" section so nothing is hidden). Without AI
  configured, digests still deliver as clean grouped lists. Per-digest custom AI
  instructions ride the same guardrails as the workspace AI-prompt system, and a
  new "Digest generation" entry appears in Settings → AI Prompts. Disabled by
  default behind `ENABLE_DIGESTS=1` while it hardens; on Campbooks Cloud it is a
  paid-plan feature.
- **Mobile folder bottom-sheet.** Tapping "Folders" in the mobile chip bar slides
  up a full-screen sheet with the complete folder list: system folders (Inbox, Sent,
  Drafts, Archive, Spam, Trash) and custom folders with their icons, message counts,
  collapsible nesting, and the rename/move/delete edit affordances that were previously
  desktop-only. The chip bar stays for fast one-tap switching; the sheet is purely
  additive. Custom folders stay live via turbo streams on create/update/delete.
- **Inbox settings, now in the dashboard.** Everything from the inbox's gear
  menu — tags, document types, filtering, smart groups, labels, signatures,
  connected accounts, and display preferences — is now also reachable from the
  settings sidebar, under a new **Inbox** group with a page per panel. The gear
  menu still works; this just gives the same controls a permanent home alongside
  your other settings.
- **Let your AI agent run your inbox (MCP).** The MCP endpoint at `/api/mcp` grew from a
  REST mirror into a full agent surface: `get_overview` / `get_setup_status` / `guide`
  (on-demand knowledge topics so agents stay lean), `search_emails` (semantic + keyword),
  bulk inbox actions (`update_emails`, `move_emails_to_folder`, `tag_emails`,
  `forward_email`), Skim triage over MCP (`get_skim_deck` / `skim_decide`, wired into the
  same learning loop as the UI), task tools, `list_calendars` / `create_event_from_email`,
  taxonomy creation (`create_tag` / `create_document_type` / `create_folder`), and email
  account tools — `list_email_accounts` plus `connect_email_account`, which can accept a
  locally-minted OAuth refresh token for self-hosted setups. Three new scopes:
  `email_accounts:read`, `email_accounts:write`, `document_types:write`.
- **MCP keys — agent credentials that don't expire.** `/api/mcp` now also accepts
  `Bearer <client-id>.<client-secret>` (and HTTP Basic) so agent configs need no token
  refresh; the Settings → API access reveal page shows a ready-to-copy "MCP key".
  Rotate the client secret (or delete the client) to revoke. REST tokens are unchanged.
- **Claude Code plugin.** `/plugin marketplace add notacamp/campbooks` installs the
  `campbooks` plugin: the MCP server preconfigured (prompts for your server URL and MCP
  key — self-hosted friendly), a guided `/campbooks:setup` onboarding skill, a
  `/campbooks:triage` daily-inbox skill, and a local OAuth helper script for connecting
  mailboxes on self-hosted instances. Config snippets for Cursor, Windsurf, Codex CLI,
  and Gemini CLI ship in the plugin README.
- **Search like you already know how.** The inbox search bar now understands
  Gmail-style modifiers — `from:`, `to:`, `subject:`, `has:attachment`,
  `is:unread/read/pinned`, `before:`/`after:`, `tag:`, `folder:`, `category:`,
  `priority:`, and `account:`. As you type, a suggestions dropdown under the
  search box offers every modifier (with a plain-language description) and then
  completes its values for you: contacts for `from:`/`to:`, your tags, folders,
  and categories, all keyboard-navigable (arrows + Enter, Escape to dismiss).
- **A quiet "searching…" signal.** While results are loading, the search icon
  becomes a spinner and the current list dims slightly — no more wondering
  whether the search actually ran.

### Changed

- **Search results are ranked by relevance, not just date.** Results now blend
  how well a message matches (subject beats sender, sender beats summary,
  semantic similarity counts too) with a light recency boost — a strong match
  from last month now outranks a weak match from this morning.
- **The home feed now ranks by real urgency, not card type.** Every card kind
  scores on one continuous "how much does this need you right now" scale —
  reminders and tasks climb smoothly as their date approaches, reply nudges
  firm up as the silence stretches — instead of jumping between per-type tiers
  that made the feed read as sections. Ranking also learns from you: card kinds
  you habitually dismiss drift down while kinds you act on rise, cards you've
  been shown for days without touching yield to fresher ones, and mail from
  senders whose messages historically run urgent gets a lift. The timeline
  additionally interleaves card types (no more walls of one kind) and folds all
  of a page's tag suggestions into a single queue row at the end.

### Fixed

- **The "Report a bug" drawer opens right away.** It used to hang for a moment
  before appearing, because it captured the page screenshot up front and that
  work blocked the drawer's slide-in. The snapshot now happens just after the
  drawer is open, so it slides in immediately (the screenshot is still attached
  to the report exactly as before).
- **Searching no longer hides your follow-ups.** The "Waiting on replies" band
  now takes part in search: threads you're waiting on that match the query stay
  visible in their own band at the top of the results (and are no longer
  silently dropped from the view).
- **Notifications no longer dead-end on a "not found" page.** Clicking an older
  notification could land on a 404 — a leftover from the switch to UUID
  identifiers, which left some notification links pointing at records by their old
  numeric id. Existing links are repaired automatically on upgrade: where the
  target is known they reopen the right document or Scout thread, and any that
  can't be recovered fall back to the relevant list instead of a dead end.  
- **Settings no longer links to pages that aren't turned on.** The Document templates
  and Email templates entries appeared in the Settings menu even in deployments where
  those features are disabled, so clicking one opened a blank page. They're now hidden
  unless the feature is enabled.
- **Stale follow-ups no longer haunt the feed.** A follow-up nudge without an
  AI-scheduled time was re-dated to "now" on every refresh, so months-old,
  clearly-irrelevant follow-ups could stay pinned in "Needs attention" forever.
  They now age from the moment you sent the mail, retire from the feed once
  truly stale (about two months), and proactive nudges stop for un-analyzed
  threads silent past 60 days — the durable "waiting on replies" list in the
  inbox is unaffected.
- **Read starred-sender mail stops pinning "Needs attention".** Mail from
  starred contacts used to hold a pinned spot for over a week even after being
  read; now it pins only while unread (or flagged urgent) and otherwise ranks
  high in the timeline.
- **Fully stale cards leave the feed instead of piling up at the bottom**, and
  a card resolved by the system (say, a snoozed thread whose snooze you extend)
  can now come back if it genuinely needs you again — previously it was hidden
  forever.
- **The calendar "Manage access" page opens again.** Opening a calendar
  account's sharing panel crashed with a template error, which made calendar
  sharing unmanageable from the UI (roles could still be changed via the API).

### Security

- **Workspace admins are no longer application admins.** ⚠️ The admin role on
  a user used to double as access to the instance-wide `/admin` panel and the
  `/jobs` dashboard — surfaces that span every workspace on the server. Those
  are now gated by a separate per-user `app_admin` flag; the role on a user
  only governs their own workspace. Existing role-admins keep instance access
  on upgrade (they're marked `app_admin` by the migration); self-hosters can
  grant it in the Rails console with `user.update!(app_admin: true)`.
- **Documents filed into restricted folders are hidden by direct link too.**
  Folder read restrictions were enforced on the Files listing but not on a
  document's own page or file download — anyone in the workspace who knew or
  guessed the URL could open a restricted document. Both now 404 unless the
  viewer can read one of the document's folders (workspace admins retain
  access).
- **Invitation controls are for the inviter and workspace admins.** Any
  workspace member could cancel or resend any pending invitation; now only the
  person who sent it or a workspace admin can.
- **Scheduled emails now follow the mailbox's sharing.** Queued sends were
  visible to every workspace member — including the recipient, subject, and
  body of mail scheduled on a teammate's private mailbox — and anyone in the
  workspace could edit or cancel them. A scheduled email is now visible only to
  its creator and to people the mailbox is shared with, and only the creator or
  someone with send permission on that mailbox can change or cancel it (web,
  API, and MCP alike).
- **"Schedule send" now checks send permission.** The composer's Schedule
  button accepted any account id, so someone with read-only access to a shared
  mailbox (or a crafted request naming any account) could queue an email from
  it. Scheduling now requires send permission on the account, exactly like
  pressing Send.

## [0.11.0] - 2026-07-05

### Added

- **Broader daily digest.** The "waiting on replies" email has been expanded into a unified "needs attention" digest. One daily email now groups three sections — follow-ups you're still waiting on, reminders that are due soon or overdue, and tasks that are overdue or high-priority — with empty sections omitted automatically. The preference toggle in Settings → Notifications gates all three sections. (#177)
- **Swipe actions on tasks and calendar events.** In the task list, swipe right
  to complete a task or left to archive it (then deeper left to delete with a
  confirmation). In the calendar agenda view, swipe left to delete a
  non-recurring, writable event. Both actions slide the row out and show a
  success toast — no page reload. Recurring events and read-only calendars are
  deliberately excluded from swipe to avoid the recurrence-scope dialog gap.
- **Feed cards open the email right there.** Cards that ask you to decide
  something — needs-attention mail, reply nudges, follow-ups, starred senders,
  and reminders or suggested tasks that came from an email — now carry a small
  "Show email" toggle that unfolds the full message (real formatting, safely
  sandboxed) inside the card. Peek, decide, archive/file/confirm — without ever
  leaving the feed. The message loads only when you open it, so the feed stays
  as fast as before.
- **Repeating calendar events.** Set how an event repeats — daily, every weekday,
  weekly, every two weeks, monthly, or yearly — right on the event form. Campbooks
  creates it as a real recurring series in Google/Zoho and shows every occurrence
  on your calendar straight away; editing or deleting still lets you choose just
  that one or the whole series.
- **Repeating tasks.** Give a task a repeat schedule and, each time you complete
  it, Campbooks lines up the next one — carrying the title, priority, labels, and
  assignees forward — so recurring chores never fall off your list.

### Changed

- **Organizations moves off the mobile bottom dock into the "More" menu.** On narrow screens the dock is five items wide; Organizations now collapses into the "More" burger alongside Tasks, Workflows, Contacts, and Activity, freeing a dock slot for the items you reach every day.
- **The home feed now ranks by priority, not by section.** Every card gets one
  score blending what it is (a due reminder, a follow-up, actionable mail…),
  how relevant it is (starred and known contacts up, newsletters and other bulk
  mail down, conversations you've written in up), and how fresh it is — scores
  decay with age, so a follow-up from two years ago sinks to the bottom instead
  of pinning above this morning's mail. "Needs attention" keeps only what's
  urgent *now*; stale items demote into the ranked timeline automatically.
- **The month calendar's day cells are calmer to use.** Adding an event is now an
  explicit "+" that fades in when you hover a day (and stays visible on touch), so
  a stray click on a day no longer starts a new event. Days holding more events
  than fit show a "+N more" — click it, or tap the row of dots on a phone, to pop
  open the whole day's list.
- The inbox search bar now stretches across the whole top band, meeting the
  Compose button instead of stopping short and leaving a dead gap on wide
  screens.

### Fixed

- **Follow-up cards now show the email you sent, not the last one you got.** The
  home-feed "Draft follow-up" card is a nudge about a message *you* sent and are
  still waiting on — but it led with the other party's name, showed their subject,
  and its "Show email" peek unfolded the mail you received, not the one you're
  chasing. Cards now read "To <recipient>", show your sent subject, and peek into
  the message you actually sent. The nudge still drafts and addresses correctly to
  the other party.
- **The mobile bottom nav hides itself while you scroll down** and glides back
  the moment you scroll up — reclaiming screen space during reading without
  making navigation hard to reach.
- **Tapping the inbox icon or a folder chip on mobile now shows the email list, not an already-open message.** The server redirect to the latest email is unchanged — on a phone or small tablet, tapping a nav item or a folder chip lands on the thread list first; tapping a row opens the reading pane as expected. Direct deep-links (push notifications, digest emails) still open straight to the email. Folder chips also gained a larger tap target and a press-down animation.
- **Swipe actions in the "Waiting on replies" band now match the row's buttons.**
  Swiping left on a waiting-reply thread now reveals "Dismiss follow-up" (the
  same action as the inline × button) rather than Archive. The right side is
  empty — "Draft follow-up" is a full-page action and stays tap-only.
- **Documents no longer show your email address as their title.** Attachments
  pulled in from received or sent email were incorrectly stored with the raw
  sender address (e.g. `you@example.com`) as their name. Any document whose AI
  analysis hadn't finished would surface that address as its title in the Files
  list. The creation paths now leave the name blank so the filename is shown
  instead, and a migration clears the stale email-address values from existing
  rows.
- **The Skim rings are back.** In 0.10.0 the skim tray on the home feed and
  inbox rendered "Content missing" for most inboxes: the live categorizer's new
  Gmail-category rescue read a column (`provider_labels`) the tray's trimmed
  query didn't load, which 500'd the whole tray. The tray query now loads every
  column the categorizer reads — including the bulk-mail headers it was
  silently missing before, so Skim's ring placement matches regular triage —
  and the Gmail hint degrades gracefully instead of erroring if a narrow query
  ever starves it again.

## [0.10.0] - 2026-07-04

### Added

- **Manage calendars right on the calendar.** A Google-Calendar-style sidebar
  lists every calendar grouped by the email account that owns it: tick a
  calendar to show or hide it **just for you**, recolor it, stop it syncing,
  enable calendars discovered at your provider that aren't syncing yet, refresh
  the provider list on demand, and jump to connecting another account. On phones
  the same pane opens from a button next to the calendar title.
- **Import events from an `.ics` file.** Export from any calendar app, pick a
  Campbooks calendar, and imported events sync out to Google/Zoho just like
  ones you create here. Re-importing the same file skips duplicates; recurring
  events are skipped (and counted) for now.
- **A daily nudge that reaches your inbox, not just the app.** With it on (the
  default), Campbooks emails you a short morning digest of the conversations you're
  still waiting to hear back on — so a dropped thread catches your eye even when
  you're nowhere near Campbooks. Turn it off anytime in Settings → Notifications.

### Changed

- **Opening an email no longer reloads the whole inbox.** Clicking a thread in the
  list now swaps just the reading pane — the list keeps its scroll position and
  your place stays in view. The command palette (⌘K), keyboard shortcuts, and the
  discussion panel all follow along to the email on screen; on a phone the same
  tap still flips you to the message as before.
- **Smart sorting now reads Gmail's own verdicts.** When Gmail has already filed
  a message under Promotions, Social, or Updates and Campbooks' own rules see no
  bulk signal, the message follows Gmail's verdict into the matching smart group
  instead of landing in your inbox list — catching newsletters and bulk senders
  that don't look automated. Gmail's "Personal" tab is deliberately not trusted
  (it's a catch-all that would pull machine mail back inline), and a provider
  verdict never overrides a security-flavored subject. Existing mail can be
  re-sorted with `bin/rails emails:categorize ALL=1 WRITE=1`.
- **The "Waiting on replies" list now checks with the AI before flagging a thread.**
  If the last message you sent was just an FYI, an acknowledgement, or a sign-off the
  other person isn't expected to answer, it no longer shows up as something you're
  waiting on — in the inbox band, the Scout count, the feed, or the digest. When no
  AI provider is configured the list still works; it just can't vet, so it shows
  everything you sent last.
- ⚠️ **Color belongs to the calendar now; event types carry an icon.** Event
  chips are always tinted with the owning calendar's color (pick it in the
  sidebar — your choice survives sync), and each event type marks its events
  with a small icon from the app's icon set instead of a color. Per-event color
  overrides and event-type colors are removed, including their columns; the
  API's `calendar_event.color` now always reflects the calendar and `color` is
  no longer accepted on event create/update.
- Events from calendars that are switched off no longer linger on the calendar
  page — off means off.

## [0.9.0] - 2026-07-03

### Added

- **Low-priority mail now folds itself out of your way.** The inbox bundles
  Notifications, Newsletters & promos, Social, and Updates threads into
  collapsed **smart group** rows — stacked sender avatars, a count, one tap to
  review the bucket, and **Archive all** / **Mark all read** to clear it in one
  go. On by default with per-bucket toggles in the inbox gear menu → Smart
  groups. Personal, important, and uncategorized mail always stays in the list,
  and threads you replied to, pinned threads, or starred senders are never
  grouped. Custom tag groups render alongside (and their rows are actually
  back — a regression had kept them from appearing at all).
- **Campbooks now keeps track of the replies you're still waiting on.** Answer
  someone and hear nothing back, and that thread rises into a new **Waiting on
  replies** band at the top of your inbox — showing how long it's been quiet, with
  a one-tap draft to nudge them — instead of sinking down the list. Scout counts
  them in its briefing too, so "what am I still waiting to hear back on?" always
  has an answer. It runs on the plain fact that you sent last, so it works even
  before you've set up an AI provider; the AI only sharpens the timing and writes
  the nudge once it's connected.

### Changed

- **A roomier, calmer calendar.** The month and week views now stretch to fill the
  whole page instead of floating in a short, narrow box, so far more of your schedule
  is visible at a glance. The week and day views open on your working hours (no more
  staring at an empty pre‑dawn grid) and their day header stays pinned as a frosted
  glass bar while the hours scroll underneath. Hovering a day in the month grid, or
  an empty slot in the week/day grid, now shows a clear "click to add" cue, and
  timed events read as a tidy time + title line rather than a wall of colored bars.
  Today is gently highlighted throughout.

- The follow-up nudges on the home feed and in Skim's Follow-ups ring now draw on
  that same signal, so they keep surfacing the threads you're waiting on even when
  no AI provider is configured.

## [0.8.0] - 2026-07-03

### Added

- **Watch Scout sort your inbox the moment you connect it.** A brand-new account now
  lands on a live first-sync stage instead of an empty feed: counters tick up as your
  first scan reads and sorts mail (found · sorted · needs you), then one tap drops you
  into the sorted feed. The first scan also starts immediately on connect instead of
  waiting for the next polling cycle.
- **Composing is its own room now.** Replying pulls up the **Dock** — a bottom
  sheet over your inbox — instead of a form wedged into the thread. Recipients
  and subject collapse to one line when they're already right, the quoted
  thread tucks behind a small pill until you need it, and formatting appears
  when you select text instead of a toolbar you never asked for. Reply,
  reply-all and forward are one switcher — flip modes without losing what you
  typed, and forwards carry the original attachments as removable chips.
- **A new email gets the whole screen.** The compose page (the **Desk**) drops
  the folder rail and message list for one centered writing surface — subject
  set like a title, an open canvas, and Scout waiting in a side rail. Pick
  where new emails open (full page or bottom sheet) in **Settings → Account**.
- **Drafts save themselves.** Everything you type autosaves; minimize the
  composer to a small pill that survives navigating anywhere in the app, and
  resume exactly where you left off — or expand a reply from the sheet to the
  full page with everything carried over. Sending or discarding cleans the
  draft up.
- **Replies can start answered.** When Scout has already suggested a reply for
  a thread, the composer opens with that draft as a glass "ghost" block — use
  it as-is, ask for a shorter/warmer/firmer take, or start blank. The spark
  button asks Scout on demand.

- **Reminders, to‑dos, and filing suggestions learn from your choices.** When Scout
  proposes a reminder, a task, or a tag to file an email under, it now watches how you
  respond. If you keep dismissing a kind of reminder from a particular sender, Scout
  stops surfacing it; a tag you keep rejecting for a sender won't be suggested again;
  and one you always accept floats to the top. It only ever backs off after a clear,
  repeated pattern from that same sender — and it only quiets a suggestion, never an
  actual email or document.
- **See how soon things are on the calendar.** The Agenda view now shows a small
  countdown next to each event and reminder — "In 20 min", "In 3 h", "Tomorrow",
  "In 4 days" — so you can tell at a glance what's imminent. Anything happening
  now, within the hour, or today is highlighted.
- **Suggested tasks now appear on the home feed.** When Scout extracts an action
  item from an email, it shows up as a "Suggested task" card with one-tap
  **Add to tasks** / **Dismiss** — no more suggestions piling up unseen in the
  tasks triage queue. Accepted tasks keep surfacing later when they become due,
  blocked, or assigned to you.
- `tasks:backfill_extraction` rake task to re-run task extraction over recent
  mail (e.g. after enabling the Tasks module), gated and idempotent.
- **Tune how Scout reads your inbox — in plain language.** A new **Settings → AI
  Prompts** page, plus a **Customize AI** button on the Tasks, Documents, and
  Reminders pages, lets you add your own guidance for how Scout extracts tasks,
  analyzes documents, spots reminders, and summarizes and tags email. Your notes
  are appended to the built‑in instructions — they never replace the output format
  or safety rules — and clearing the box restores the default.

### Changed

- **Onboarding is one screen now, not homework.** After signup you meet Scout, connect
  an inbox, and that's it — workspace details, tax IDs, document types, and tags no
  longer stand between you and the product. The full setup wizard survives behind
  "More setup options" and in Settings.
- **The getting-started checklist no longer owns your home screen.** It's a quiet
  "Scout can do more" card riding along in the feed, capped at three next steps, and
  finished tasks simply leave the list. Document types and tags now lead with Scout
  drafting them for you.
- **Sign-in and signup got a face-lift.** Flat, confident screens instead of boxed
  gray forms — and the email verification step is now a six-box code input that
  advances, accepts pasting, and submits itself on the last digit.
- **The product tour reads like a demo, not a slideshow.** Scenes settle in the middle
  of the screen with staged entrances, progress is six dots, Scout appears as a named
  presence on its ember note, and the finale actually celebrates. It's offered as
  "See it in action first" before you connect.
- **First-run nudges know their place.** The skim-rings and Scout coachmarks no longer
  fire on a never-connected home, and the greeting no longer claims "all clear" before
  an inbox exists.
- **The inbox now speaks the app's design language.** The mail view sheds its
  legacy three-pane, hairline-fenced look: date/priority group bars are now
  floating frosted pills the list scrolls under, thread rows hover and select as
  soft rounded fills instead of edge-to-edge blocks, the unread dot is the warm
  Ember signal (matching the nav-rail attention dot) instead of blue, and the
  search-and-rings band sits on the open canvas so the work panes read as
  elevated surfaces in dark mode. Scout's read on an open email is now its full
  Ember-glass contribution block — avatar, name, AI tag, and suggested actions
  together — rather than a one-line caption, and floating chrome (bulk-select
  menus and toolbars, the reading drawer's docked footer) is frosted like the
  app's toasts and Scout bar.
- **The calendar opens on the Month view by default** (was Agenda), giving you the
  whole month at a glance when you land on the page. You can still switch views
  from the tabs, and any bookmarked `?view=…` link is unchanged.
- The Skim triage learning that already remembered your keep/archive/promote habits
  now runs on a shared, reusable foundation (no change to how Skim behaves).

### Fixed

- **Home no longer crashes for accounts with no starred contacts** (an empty-set
  sentinel in the feed's look-back query assumed uuid primary keys).
- **The Organizations directory fills itself in.** *Sync from contacts* could come
  up empty even for a workspace with thousands of contacts, because a contact only
  becomes an organization after Scout has analyzed it — and that analysis was never
  guaranteed to run for a mailbox connected with existing history, or one where AI was
  turned on after import. A background catch‑up now analyzes those skipped contacts on
  its own, so the directory populates and keeps up with no action from you.
- **Keyboard shortcuts no longer scramble the inbox.** After moving between emails
  with the arrow keys, pressing a shortcut (archive, reply, forward…) could leave the
  inbox unstyled — plain text with underlined links — until you reloaded. Shortcuts
  now always act on the email you're actually reading.
- **The Compose button now sits flush right in the inbox toolbar** instead of drifting
  toward the middle on wide screens.
- **Emails opened in the bottom-right drawer are now marked as read.** Opening a
  message in the drawer (the List and Board inbox layouts) clears its unread dot and
  updates your unread counts, just like opening it in the full reading view.
- **Email bodies are legible in dark mode again** — message-bubble text could
  render dark-on-dark in the conversation view; it now uses theme-safe tokens.
- **Task extraction now targets mail from people, in your language.**
  Extraction skips automated senders and machine mail (notification digests,
  code-review bots, no-reply security alerts, marketplace CTAs) — previously
  these produced streams of nonsense tasks — and the pre-filter recognises
  action requests in Portuguese, Spanish and French, not just English, so real
  asks from real people no longer slip through unextracted. Suggestions already
  minted from machine mail are dismissed automatically on upgrade.
- The same ask repeated across an email conversation now yields **one** task:
  replies are analysed quote-stripped, extraction dedupes per thread (not per
  message), and the model is shown the conversation's already-tracked tasks.
- Task and reminder extraction no longer silently lose an email's results when
  the AI provider rate-limits or errors transiently — the job now retries.
- Sent mail no longer generates tasks for you from requests **you** made of
  someone else.
- AI extraction (tasks and reminders) no longer wastes its reading window on
  raw `<style>` CSS from HTML email — extractors now see clean message text.
- **Command-palette email actions** (Reply, Archive… from ⌘K on an open
  email) had the same stale-id truncation bug as the keyboard shortcuts —
  fixed alongside.

## [0.7.0] - 2026-06-30

### Added

- **Email threads read like a conversation.** Messages in a thread now render as
  light, directional chat bubbles — the ones you received on the left, the ones you
  sent on the right — so it's clear at a glance who said what. Folded messages show
  a one‑line preview, and long or wide HTML emails still scroll neatly inside their
  bubble. Prefer the old look? Switch **Conversation view** to **Classic** (a flat
  list) in **Inbox settings → Display**.

- **Approve or dismiss Scout's reminders right in the thread** — when Scout spots a
  dated commitment in an email it now surfaces it in the discussion as *a potential
  reminder* with **Add to calendar** and **Dismiss** buttons, so you confirm it onto
  your calendar or wave it off in one tap. The buttons match the home‑feed reminder
  card, and acting on one collapses both into a quiet "added" / "dismissed" note.

- **Smart search on the Files page.** A new search box finds documents by meaning,
  not just exact words — search the way you'd describe it ("the contract with Acme",
  "all payment receipts to EDP", "invoice FT 2024/123") and the most relevant files
  rank first. It blends semantic (vector) matching with exact keyword lookups and
  recognises document types, company names, and invoice/receipt numbers in English
  and Portuguese. To power it, the AI now writes a short, search-optimized summary
  when it analyzes a document. Existing files keep working; self-hosters can re-index
  them for the richer search with `bin/rails search:reindex_documents`.

### Changed

- **A more breathable inbox.** The thread list has more room to breathe — larger,
  more legible type, roomier rows, and clearer (but still calm) hover and selected
  states across all three densities. **Compact** mode's text is no longer tiny.

- **Inbox top bar.** The Skim ring tray now spans the full width above the list and
  reading panes, so the rings breathe instead of scrolling in a narrow column, and
  the inbox's main actions — **Search** and **Compose** — move up to a top toolbar
  where primary actions belong.

### Fixed

- **List view opens the reading drawer again.** Clicking an email in the inbox's
  **List** layout now opens it in the bottom‑right drawer instead of navigating the
  whole page (the row‑click matcher only recognised numeric ids, so it never matched
  the app's message ids). The standalone inbox also gained the Default / List
  switcher, so the layout applies there too.

## [0.6.0] - 2026-06-29

### Added

- **Files — sharing, public links & Scout updates** — restrict a folder to chosen
  people with viewer / editor / manager roles from **"Manage access"**; a restricted
  folder and its contents stay hidden from everyone else, while open folders remain
  visible to the whole workspace. Create a **revocable public link** to any file and
  **insert it into an email** from the composer's "Insert file link", or paste one
  into a discussion comment (bare links are now clickable there). And, opt-in per
  workspace (Settings → Data & privacy), **Scout posts a link to a document into its
  email's discussion** once that document is filed.

### Changed

- **Documents and Files are now one page.** The separate "Documents" area has
  merged into **Files** — the same folder-based file manager now also holds the
  document review queue (a **"Review N"** button opens Skim right there), the
  document filters (type, category, status, month; a filter also narrows the
  internal documents and filed emails shown alongside), and the bulk
  re-analyze / export. Uploads gained an **"Analyze with AI"** toggle: leave it on
  to extract and classify a business document, turn it off to just store a file.
  The old `/documents` address now opens Files.
- **Email labels are smarter and less noisy.** Labels synced from Gmail/Zoho are
  now evaluated once on import: built-in provider statuses (Inbox, Unread,
  Important, Gmail's category tabs like Updates/Promotions) and low-value labels
  are recognised and kept out of your inbox, while the labels you actually use
  (Invoices, Clients, …) stay. The decision is remembered per label — provider
  system/noise labels are no longer attached to every message.
- **One place for tags.** "Tags" and provider "labels" are now a single concept in
  the inbox: one Tags row per email and one picker that adds or removes either a
  Campbooks tag or a Gmail/Zoho label (provider labels still two-way sync). The
  redundant separate "Labels" section is gone, and hidden provider/system labels
  never show as chips. Also fixes tag add/remove on installs using UUID message
  IDs.
- **Review & override hidden labels.** Settings → Tags now lists the labels that
  were hidden — provider system statuses and AI-filtered ones — with the reason
  for each, and a one-click **Show** to bring any back as a tag (or **Hide** a tag
  you don't want as a chip). This replaces the old global "Show system labels"
  switch in Display settings.

### Security

- Updated the bundled `msgpack` gem to 1.8.3, fixing CVE-2026-54522 (a
  use-after-free in the gem's C extension that bundler-audit flagged).

## [0.5.0] - 2026-06-29

### Added

- **Files** — a native file area in the main nav for keeping your documents and
  files organized. Upload any file, create folders, browse a folder, and move files
  between folders. Files uploaded here are stored as-is — they're not run through
  the document AI analysis (you can still send one through it later from its page) —
  and because folders are shared with Mail, a folder can hold both files and emails.
  Uploads, folder changes, and filing all show up in your workspace Activity.
- **Files — internal documents & emails in folders** — write rich-text **internal
  documents** right in Files ("New document") and file them into folders, and **file
  emails into folders** too — so a folder can hold uploaded files, internal documents,
  and emails side by side. Each is listed in the folder and recorded in Activity.
- **Tasks** — a new task-management module (opt-in via `ENABLE_TASKS`, gated by the
  `tasks` plan entitlement). Create tasks manually or have Scout extract action items
  from your email and documents (triaged in Skim, with the originating email and
  Scout's reasoning shown). Move tasks through a drag-and-drop status board; assign
  multiple workspace members; label them with the same tags as email; set due dates,
  rich-text descriptions, and a linked deadline reminder; link tasks to emails (typed
  relationships) and attach documents; archive or delete tasks; and discuss them in a
  thread where Scout joins on `@scout`. Tasks publish domain events (`task.created`,
  `task.status_changed`, `task.assigned`, `task.completed`, `task.archived`), appear in
  the navigation, Skim, and Feed, and are exposed over the public REST API
  (`tasks:read` / `tasks:write`).
- **Scout notes events & reminders in the email discussion** — when a calendar
  event is created from an email, or Scout extracts reminders from one, Scout now
  posts a short message into that email's discussion thread linking back to the new
  event/reminder, so the discussion is a running record of what Scout did with the
  email. Reminder notes are limited to confident finds to keep the thread quiet.
- **Email templates** — reusable, AI-draftable email templates (opt-in via
  `ENABLE_EMAIL_TEMPLATES`, gated by the `email_templates` plan entitlement).
  Manage them at Settings → Email templates (subject, rich-text body, and attached
  document templates that render to PDFs), generate a first draft with AI, then pull
  a template into the composer through a variable-fill picker. A template can also
  back a scheduled send — its subject/body re-render their Liquid variables and the
  attached PDFs regenerate on every occurrence. Exposed over the public REST API
  (`templates:read` / `templates:write`) and to Scout/MCP (`list_email_templates`).

### Changed

- **Native apps hide desktop-only surfaces** — the iOS/Android shell no longer
  shows the ⌘K command palette or the keyboard-shortcut help (both keyboard-only),
  limits the calendar to **Agenda/Day** (the week/month grids are too dense for a
  phone — a week/month deep link falls back to agenda), and hides developer-only
  Settings (**API Access** and custom HTTP **Connections**). The web app is
  unchanged.
- **Native apps are sign-in-only** — the iOS/Android apps no longer offer in-app
  account creation. The sign-in screen points new users to the web instead of the
  in-app signup, and the registration flow is blocked in the native shell (invited
  users can still finish onboarding in-app). This keeps web-based subscription
  billing outside Apple/Google in-app purchase.

### Fixed

- **Activity feed** — the "Pipelines" filter no longer failed to render its label.
- **Scout no longer doubles up calendar events or reminders from the same email.**
  Creating an event from an email is now idempotent — the reminder card, Scout's
  "Create event" button, and repeated clicks resolve to a single event instead of
  stacking duplicates — and Scout now sees the commitments already extracted from a
  thread, so it acknowledges them rather than re-suggesting. An invoice that arrives as
  both an email and its PDF attachment now stages one reminder, not two.
- Documents list **month filter** now works. The month picker submits a single
  `YYYY-MM` value, but the list, "Reanalyze all", and export were looking for a
  separate `year` parameter that no form ever sends — so picking a month had no
  effect. They now parse the picker value correctly.
- `document_templates` was missing from `db/schema.rb`, so fresh installs and CI
  databases (built via `schema:load`) never got the table — and because the load
  also marks the migration as applied, `db:migrate` wouldn't re-create it. This
  broke the document-templates feature on a new install when enabled. The table
  is now in the schema (existing/upgraded databases already have it from the
  migration).
- **Skim keyboard shortcuts no longer leak to the screen behind it.** With a Skim
  overlay open, pressing a key (e.g. `e` to archive, `c`, or the arrows) also fired
  the matching inbox/feed shortcut underneath — archiving, composing, or navigating
  the wrong thing. Skim now keeps the keyboard to itself while it's open.

### Security

- **Scheduled emails** now resolve the "from" account against the accounts you're
  actually allowed to send from — on both the web form and the REST API — and assign
  it explicitly instead of accepting it through mass assignment. A tampered request can
  no longer attach a schedule to an email account in another workspace, and the web
  form rejects a disallowed account immediately rather than letting it fail later at
  send time.
- Hardened the onboarding wizard's step dispatch so the step name can only ever come
  from the fixed list of steps (defense-in-depth around dynamic method dispatch).

## [0.4.0] - 2026-06-28

### Added

- **Campbooks CLI + browser sign-in** — a new developer CLI (`campbooks`) drives
  the public REST API from your terminal. `campbooks login` uses a new OAuth 2.0
  **authorization-code + PKCE** grant (a styled consent screen at
  `/api/oauth/authorize`) so you sign in through your browser instead of pasting
  API keys; existing client-credentials clients are unchanged. See
  [docs/cli.md](docs/cli.md) and [docs/api.md](docs/api.md).
- **Calendar event types** — create calendar-only "types" (each a name, a color, and
  an AI prompt) from the new **Event types** button on the calendar. New events are
  auto-classified into a type and colored to match — both events created from email
  and ones you add yourself — and you can always override the type (or choose "None")
  on the event form. A one-click starter set gets you going, and the type's color
  syncs out to Google/Zoho.
- **Drag events across days in month view** — drag an event from one day to another
  in the calendar's month view to reschedule it; the time of day and duration are
  preserved. Works on touch as well as desktop.
- **Public REST API — new resources** — added endpoints for scheduled emails
  (`scheduled_emails:read`/`:write`), calendar events (`calendar:read`/`:write`),
  reminders (`reminders:read`/`:write`), and folders with folder-membership
  filing (`folders:read`/`:write`). See [`docs/api.md`](docs/api.md) /
  `openapi.yaml`.
- **MCP endpoint** (`POST /api/mcp`) — exposes the full public API as a Model
  Context Protocol (JSON-RPC 2.0) server, authenticated with the same bearer
  token as the REST API. Tools mirror the REST surface one-for-one across email
  (incl. tags), documents, contacts, tags, document types, workflows, Scout chat,
  scheduled emails, calendar events, reminders, and folders — each gated by its
  REST scope, so `tools/list` only returns the tools a token may call.
- **Credit Note document type** — "Nota de Crédito" (NC) is now a first-class
  document type instead of being filed under expense invoices. Documents the AI
  recognises as credit notes are classified, labelled (en/pt/es/fr), and filtered
  under Accounting as their own type, with a dedicated extraction schema
  (credit-note number, original invoice number, amounts, IVA).
- **Scout can now think.** The global Scout chat shows a collapsible reasoning
  trace and the tools it ran ("Searched email → 12 results") above each answer,
  using the model's native reasoning where the configured model supports it
  (Claude extended thinking, OpenAI/DeepSeek reasoning) and degrading cleanly
  otherwise.

### Changed

- **Global Scout rebuilt on native tool calling.** Replaced the hand-rolled
  "emit JSON in prose" protocol with the providers' native function/tool-calling
  APIs, a single JSON-Schema tool registry (one source of truth, validated
  before execution), and a model-driven loop that runs until Scout has a real
  answer instead of erroring out after a fixed number of tool calls. Destructive
  actions are never executed from model output — they're surfaced as one-click
  confirmations.
- ⚠️ **All record identifiers are now UUIDs.** Primary keys across every domain
  table moved from sequential integers to UUIDs, so ids in URLs and in the public
  REST API are now non-sequential uuids (e.g. `/documents/9d94f3a1-…` instead of
  `/documents/42`). Existing integer-id URLs, bookmarks, and stored API ids will
  no longer resolve. Self-hosters: a single upgrade migration rewrites every
  primary and foreign key in one transaction.

### Security

- Bump the transitive `crass` dependency to 1.0.7, clearing four CSS-parser
  denial-of-service advisories (no behavior change).

## [0.3.0] - 2026-06-28

### Added

- **Document writing tool** — author formatted documents from scratch at
  **Documents → Write**. Built on the shared rich-text editor with a
  document-focused toolbar (tables, font family, text highlight, and
  super/subscript); the existing email-compose and signature editors are
  unchanged. Saved documents are listed, viewable, and re-editable, scoped to
  your workspace.
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
- **Document templates** — create reusable document templates whose HTML is
  generated by AI from a plain-language description, fill them with variables, and
  send the result as a PDF email attachment (Settings → Document templates). Off by
  default; enable with `ENABLE_DOCUMENT_TEMPLATES=1` (a paid feature on the hosted
  cloud). PDF rendering uses headless Chromium via Grover — build the image with
  `--build-arg INSTALL_PDF_BROWSER=1` to include it (see `docs/self-hosting.md`);
  without it the feature degrades gracefully instead of erroring.
- **Google Drive folder picker** — the Drive config form now offers an
  interactive folder browser (browse, select, or create folders) instead of
  requiring a pasted folder ID. The selected folder path is stored and
  displayed as a human-readable label.
- **Drive push status in document list** — a Drive column in the documents
  table (desktop) and a Drive badge on document cards (mobile) show whether
  a document has been pushed to Drive (green checkmark), failed (amber warning
  with one-click retry), or hasn't been pushed yet.
- **Retry all failed Drive pushes** — the Google Drive settings page now shows
  a count of failed uploads and a "Retry all" button that re-enqueues every
  failed document in one click.
- **Sent-email attachments** — files attached to emails you send are now stored
  locally as documents (the same way received attachments are), and the AI
  biases their classification toward revenue/outgoing types.
- Configurable pipeline kanban boards for documents and emails. Workspaces can
  define custom stages, drag items between them, and hook stage transitions into
  workflows. Gated by plan (1 pipeline on Free, 5 on Pro, unlimited on
  Business/Unlimited).

### Changed

- **Google Drive auto-push now defaults to on** — when you configure a Drive
  destination for a document type, approved documents are uploaded automatically
  unless you explicitly pause it. The checkbox label now reads "Upload
  automatically when approved." A migration flips the column default.
- Sidebar navigation attention dots now reflect whether a section still has
  something for you to look at — unread mail, new feed items, pending reminders,
  documents awaiting review, or unread Scout replies — and clear when you handle
  the resource from any surface (home feed, skim, mail, or Scout), rather than
  being tied to when you last opened that section's page.

### Fixed

- Avatar stacks (facepiles) in the email list and board view now show the
  account-color ring, consistent with single-sender avatars and search results.
- **Mobile usability** — fixed several small-screen issues found auditing the app
  at 320–375px: the calendar/reminders view tabs no longer push the page into
  horizontal scroll (they scroll within their own strip), the inbox settings
  dialog stacks to a single column with a scrollable section strip instead of
  crushing its content pane, and a number of undersized tap targets were enlarged
  (the compose and email "back" buttons, the email reply/forward and Discussion
  buttons, and the tag-remove "×").

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

[Unreleased]: https://github.com/notacamp/campbooks/compare/v0.28.1...HEAD
[0.28.1]: https://github.com/notacamp/campbooks/compare/v0.28.0...v0.28.1
[0.28.0]: https://github.com/notacamp/campbooks/compare/v0.27.0...v0.28.0
[0.27.0]: https://github.com/notacamp/campbooks/compare/v0.26.2...v0.27.0
[0.26.2]: https://github.com/notacamp/campbooks/compare/v0.26.1...v0.26.2
[0.26.1]: https://github.com/notacamp/campbooks/compare/v0.26.0...v0.26.1
[0.21.1]: https://github.com/notacamp/campbooks/compare/v0.21.0...v0.21.1
[0.21.0]: https://github.com/notacamp/campbooks/compare/v0.20.0...v0.21.0
[0.20.0]: https://github.com/notacamp/campbooks/compare/v0.19.9...v0.20.0
[0.17.0]: https://github.com/notacamp/campbooks/compare/v0.16.0...v0.17.0
[0.16.0]: https://github.com/notacamp/campbooks/compare/v0.15.0...v0.16.0
[0.15.0]: https://github.com/notacamp/campbooks/compare/v0.14.0...v0.15.0
[0.14.0]: https://github.com/notacamp/campbooks/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/notacamp/campbooks/compare/v0.12.2...v0.13.0
[0.12.2]: https://github.com/notacamp/campbooks/compare/v0.12.1...v0.12.2
[0.12.1]: https://github.com/notacamp/campbooks/compare/v0.12.0...v0.12.1
[0.12.0]: https://github.com/notacamp/campbooks/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/notacamp/campbooks/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/notacamp/campbooks/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/notacamp/campbooks/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/notacamp/campbooks/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/notacamp/campbooks/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/notacamp/campbooks/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/notacamp/campbooks/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/notacamp/campbooks/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/notacamp/campbooks/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/notacamp/campbooks/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/notacamp/campbooks/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/notacamp/campbooks/releases/tag/v0.1.0
