# Design

## Owner decisions (2026-10-25)

- Pages and tables both, built like Notion: "we're not asking for much
  different than notion". Mainly for **creating documents and pages**, so
  pages ship first and tables come in change 3.
- Pages connect to the rest of the app: `@` mentions of mail, people and tasks;
  Scout reads and writes; mail saved to a page; shared with the team.
- The prototype was approved: https://claude.ai/artifact/B4DKyoW5itjeAhcgAaY2rs

## Language

- **Page**: something a member writes, in the tree. _Avoid_: document (in
  `documents` that means a file Campbooks has read), note (taken by the person
  notes of `conversations`), doc.
- **Block**: one piece of a page's body (paragraph, heading, to-do, …).
- **Mention**: an inline pointer to a record (`RecordRef`) inside a page.
- **Embed**: a block that shows a record (a thread, a task, an event).
- **Backlink**: a page that mentions or embeds a record, seen from that record.
- **Place**: where a top-level page lives, `private` or `workspace`. _Avoid_:
  space, teamspace.
- **Trash**: pages deleted in the last 30 days.

## Alternatives considered

**Editor.**

- *TipTap on ProseMirror* (chosen). MIT core and the extensions we need
  (StarterKit, TaskList, Placeholder, Collaboration, Mention, Link, Image,
  Table later). Schema-first, so the server can check the same block names.
  Its Yjs binding (`y-prosemirror`) is mature, and it renders through our own
  React components and tokens. Pro extensions (comments, AI, pages) are paid
  and **not used**.
- *BlockNote*. It is the most Notion-like out of the box, but its look comes
  with its own (Mantine) styles, which fight the token-only design system. Its
  XL packages are GPL or commercial, and it sits on TipTap anyway.
- *Lexical*. Good and small, but its Yjs binding and extension catalogue are
  thinner, and we would write more blocks ourselves.
- *Our own contenteditable*. Rejected: years of edge cases (IME, selection,
  paste, mobile keyboards).

**Body storage.**

- *A Yjs document per page, merged by `yrs` on the server* (chosen). Two
  writers never overwrite each other, an offline or slow client catches up by
  state vector, and the server can derive every projection (title, text,
  mentions) from the merged document. `yrs` is MIT and crates.io-only.
- *ProseMirror JSON with a revision and a 409 on conflict*. Simplest, but a
  shared page with two editors loses work or nags. Rejected for workspace
  pages.
- *Blocks as rows* (Notion's own model). Gives per-block access and queries we
  do not need, and turns every keystroke into row writes. Rejected; tables in
  change 3 store rows as pages, not blocks.

**Sync transport.**

- *HTTP sync plus the change feed* (chosen). `pages.sync` sends the client's
  pending update with its state vector and returns what it misses. The change
  feed (SSE) announces that a page changed, and open editors pull. That needs
  no new protocol, and it passes through the BFF, Tauri and the rate limiter as
  every other call does. A second writer's typing shows within about a second.
- *A WebSocket per open page* (y-websocket). Needed for live cursors, which are
  a non-goal. It can be added later on the same document without changing
  storage.

## Data (schema `pages`)

All tables use forced row-level security with `workspace_isolation`, like
`tasks`.

- `pages.pages`: `id` (uuid v7), `workspace_id`, `parent_id` (null = top
  level), `place` (`private` | `workspace`, set on top-level pages and
  inherited by sub-pages), `position` (fractional-index text, ordered among
  siblings), `title` (projection, ≤ 200 characters, cleaned like task titles),
  `icon` (one of a fixed set of icon names, or null), `created_by`,
  `created_at`, `edited_by`, `edited_at`, `trashed_at`, `trashed_by`,
  `trash_root` (the page whose deletion trashed this one), `revision`
  (bigint, increases with every change). Depth is capped at 12 levels, and a
  move can never put a page under itself.
- `pages.updates`: `page_id`, `seq` (bigint per page), `update` (bytea, Yjs
  v1 update), `author`, `at`. Append-only between compactions.
- `pages.snapshots`: `page_id`, `state` (bytea, a full Yjs update), `upto_seq`.
  A job compacts updates into the snapshot after 200 updates or 10 minutes
  idle, inside one transaction, so a reader always sees snapshot + tail.
- `pages.grants`: `page_id`, `member_id`, `role` (`reader` | `writer` |
  `manager`), `granted_by`, `at`. Unique on (`page_id`, `member_id`).
- `pages.links`: `page_id`, `ref_type`, `ref_id`, `kind` (`mention` | `embed`
  | `child`), unique per triple. This is a projection rebuilt from the
  document on every accepted update, and it answers backlinks.

The text projection used by search is derived on demand by the `SearchSource`
port from snapshot + tail (no stored copy).

## The block schema

One schema, shared by the client (TipTap extensions) and the server (an
allowlist of Yjs XML element and attribute names):

- Blocks: `paragraph`, `heading` (`level` 1–3), `bulletList`, `orderedList`,
  `listItem`, `taskList`, `taskItem` (`checked`), `toggle` (`open`),
  `blockquote`, `callout` (`tone`: `neutral` | `info` | `warning`),
  `horizontalRule`, `codeBlock` (`language` from a fixed list), `image` and
  `file` (`blob`: a kernel blob id, `name`, `size`), `pageLink` (`page`),
  `embed` (`ref_type`, `ref_id`).
- Inline: text with marks `bold`, `italic`, `strike`, `code`, `link` (`href`:
  `https:`, `http:` or `mailto:` only), and the `mention` node (`ref_type`,
  `ref_id`).
- `ref_type` is one of `workspaces.member`, `conversations.conversation`,
  `tasks.task`, `schedule.event`, `pages.page`, or `person`. Unknown types are
  refused.
- **People before `parties`.** Next has no person record yet (`parties` is an
  M1 module still to build). A correspondent is mentioned as `person` with
  `ref_id` = the plain, lower-cased address (the same cleaning as person
  notes). It is picked by name from the caller's own correspondents (the
  compose suggestions operation, #304), never typed. It renders with the name
  the *viewer's* mail gives that address, else the address. When `parties`
  ships, a migration rewrites `person` mentions to party `RecordRef`s. The
  address is page content the author chose to write, so it is readable by the
  page's readers, but like all page text it never goes into events, logs or
  job payloads.

The server refuses an update (400, nothing stored) that adds an element or
attribute outside the allowlist, a link with another scheme, an update over
256 KiB, or one that would make the document larger than 4 MiB or 20,000
blocks. An image or file block may only name a blob of this workspace that
the caller uploaded or that is already on this page. The block's blob
reference is held through the kernel's blob references, so the sweeper never
removes a blob a page still uses.

## Access (ADR 0017)

Resource type `pages.page`, workspace tenancy:

- **Private page**: its creator is the owner and holds every action.
- **Workspace page**: every member may `read` and `write`. The creator and
  workspace owners may also `share`, `delete` and `move` it out of the
  workspace place.
- **Grants**: a grant on a page applies to it and to every sub-page that has
  no grant for that member. `reader` reads; `writer` also writes, creates
  sub-pages and moves within what they can write; `manager` also shares and
  deletes. The `ResourceFacts` port walks the ancestors (a recursive CTE, at
  most 12 levels) and answers the caller's highest role.
- Seeing is `read`: a page the caller cannot read is a 404 for every action,
  and it is left out of `pages.list`, search and backlinks. A tree shows a
  readable sub-page under an unreadable parent as a top-level entry under
  "Shared with you".
- **Mentions do not grant.** A page stores only the `RecordRef`. The client
  renders a mention by calling the record's own read operation with the
  viewer's token, and shows "Something you can't open" on a 404. The server
  never copies a mentioned record's subject, name or text into the page,
  `pages.links`, events or search.
- **Backlinks** return only pages the caller may read, and only for a record
  the caller may read (otherwise 404).
- Client tokens get the member's access through `ClientDirectory`, with
  `pages:read` / `pages:write` scopes, like `tasks`.

## Operations

All under `/api/v1/w/{workspace_id}/pages`; reads need `pages:read`, writes
`pages:write`. Every write carries an `x-campbooks` undo where one exists,
follows idempotency keys, and is rate-limited like other writes.

| Operation | Does | Undo |
| --- | --- | --- |
| `pages.list` | The readable tree: id, parent, place, position, title, icon, `has_children`, role, edited_at. `place`, `parent`, `trashed` filters. | — |
| `pages.get` | Metadata, the caller's role, and the document as one Yjs update with its state vector. | — |
| `pages.create` | Title, icon, parent or place, position, optional initial document. | `pages.trash` |
| `pages.update` | Title, icon (with `revision`, 409 on a stale revision). | previous values |
| `pages.move` | New parent or place, and position; cycle and depth checks; leaving `workspace` needs `share` on the page. | previous parent and position |
| `pages.sync` | Body `{ state_vector, update? }`: checks and applies `update`, refreshes projections, returns `{ update: missing, state_vector }`. | client-side (Yjs undo) |
| `pages.trash` | Trashes the page and its sub-pages. | `pages.restore` |
| `pages.restore` | Restores a trash root and everything it trashed; to the top level of its place if the parent is gone. | `pages.trash` |
| `pages.purge` | Permanent delete of a trash root (manager only, confirmed in the UI). | none, stated in the dialog |
| `pages.share` / `pages.unshare` | Grant or remove a member's role (members of this workspace only). | the opposite |
| `pages.backlinks` | For a `RecordRef`: readable pages that mention or embed it. | — |

A daily job purges trash roots older than 30 days, and the workspace purge
handler removes the whole schema's rows for a deleted workspace.

## Events

- `pages.page_changed.v1`: `{ page_id, revision, parent_id, place, trashed,
  kinds: [meta|body|share|move|trash] }`, with ids and enums only, no title
  and no text (payload gate). It is published in every write's transaction.
  The change feed announces it for the page's record type, and `search`
  consumes it to reindex.
- No event per keystroke: `pages.sync` publishes at most one `body` change per
  page every 5 seconds (later updates in the window are folded into the next
  one), and editors still pull on the first.

## Search

`pages` implements `SearchSource` for record type `pages.page`: title (weight
A) and the plain text of the blocks (weight B, mentions as nothing, code
blocks included), in the workspace's language configuration (ADR 0016). At
query time, access is answered by the `Visibility` port as `pages.page:<id>`
read. Backfill pages through the port. A trashed page leaves the index, and a
restored one comes back.

## Tracked events (ADR 0020) and Coverage

Tracked events added (properties are ids, enums and counts only):

- `page_created` (`place`, `depth`, `from`: `tree` | `slash` | `shortcut`)
- `page_edited` (debounced per page per member per 10 minutes; `blocks_added`
  bucket)
- `page_shared` (`role`, `target`: `member`)
- `page_moved` (`across_places`: bool)
- `page_trashed` / `page_restored` / `page_purged`
- `mention_inserted` (`ref_type`)

Coverage rows: one per operation above, the compaction job, the trash purge
job, the workspace purge handler, and the search source handler.

## Client

Module `clients/app/src/modules/pages`, registered in `app/modules.ts`:

- **Routes** `/w/$workspaceId/pages` (opens the last page, or the empty
  state) and `/w/$workspaceId/pages/$pageId`.
- **Nav** "Pages" with `g p`. New page `n p` (a global sequence, like `n t`).
  `⌘K` finds pages through search.
- **Tree**: Workspace, Private, Shared with you; expand and collapse, drag to
  reorder and nest, "New page" and "Add a sub-page" on hover, the trash at
  the bottom. On phones the tree is its own screen and a page opens over it.
- **Editor**: TipTap with the schema above, and Collaboration over a `Y.Doc`
  synced through `pages.sync`; the change feed triggers pulls. Pending
  updates retry with backoff; the top bar shows Saved, Saving… or "Not saved:
  retrying".
  - The slash menu (`/`), with Basic and "From Campbooks" groups, filtered by
    typing.
  - `@` mentions, searched by name across people, threads, tasks, events and
    pages. A person or thread is never inserted from a typed address (person
    pickers search by name).
  - Markdown shortcuts (`#`, `-`, `1.`, `[]`, `>`, ```` ``` ````, `---`).
  - A block handle to drag, turn into another block, duplicate and delete.
  - Paste of HTML and Markdown is sanitised to the schema.
  - Files and images upload through the kernel blob store.
- **Mentions and embeds** render through each module's public read hook.
  Unknown or unreadable records show "Something you can't open".
- **Share** popover as in the prototype: members by name with a role, the
  place line, and the note that mentions keep their own access.
- **Backlinks** ("Linked from N places") under the page.
- **Undo toast** for create, move, trash, restore and share. Text editing uses
  Yjs undo (`⌘Z` / `⇧⌘Z`).
- **Phone**: a single column, a toolbar attached to the keyboard (`/`, `@`,
  to-do, bold, italic), and wide blocks that scroll inside their box. It must
  pass `pnpm test:fit` at 375 and 1280 in every locale.
- **i18n**: en, pt-PT, es-ES and fr at parity, including the slash menu, the
  block names and the placeholders.
- New dependencies, pinned exactly: `@tiptap/react`, `@tiptap/pm`,
  `@tiptap/starter-kit` and the MIT extensions named above, `yjs`,
  `y-prosemirror`, `fractional-indexing`. On the server: `yrs`. Licences are
  checked by `cargo deny` and the npm licence gate (ADR 0007).

## The flag

`CAMPBOOKS_PAGES__ENABLED` (default `false`). When off: no Pages place or
shortcuts, the operations answer 404, the search source indexes nothing, and
the jobs do nothing. When off, a stray or missing pages setting never stops
the server; this is tested against production values. The client learns it
the same way it learns which connectors are offered.

## Risks

- **Yjs update validation.** Checking a binary update against the allowlist
  means decoding it into a scratch copy of the document with `yrs` and walking
  only the changed parts. This is bounded by the 256 KiB update cap. Tests
  fuzz forged updates (unknown elements, `javascript:` links, oversize).
- **Projection cost.** Re-deriving links and title per accepted sync is
  bounded by the 4 MiB document and done after applying in the same
  transaction. If it shows in Loki timings, debounce it like the event.
- **Compaction races.** Snapshot and tail are read in one statement, and
  compaction deletes only `seq <= upto_seq` in its own transaction.

## Deviations while building

- **`meta_revision`.** `pages.update` and `pages.move` check a separate
  `meta_revision` (title, icon, place, position), not `revision`, which grows
  with every body change; otherwise typing would make renaming conflict.
- **Two extra operations**, `pages.upload_file` (raw bytes, 20 MiB, held by
  the page through `pages.files` and the kernel's blob references) and
  `pages.get_file` (raster images inline by their first bytes, everything
  else as an opaque download). A file block may only name a blob uploaded to
  its page or by the caller.
- **A `purge` kind** on `pages.page_changed`, so purging is announced as a
  delete to the change feed and to search.
- **Icons** are the design system's icon names (`document`, `tasks`, ...).
- **Mentions** are read through each record's own HTTP read operation with
  the viewer's token, not by importing other modules' hooks.
- **`@tiptap/y-tiptap`** (TipTap 3's fork of y-prosemirror, same document
  format) replaces `y-prosemirror`.
- **Schedule.** The daily trash purge is scheduled only while pages are on;
  compaction, the workspace purge and the departed-member handler stay
  registered (they only act on rows that exist).
- **`deny.toml`** ignores RUSTSEC-2026-0215 (smallstr, unmaintained, only
  through `yrs`).
- **Moving.** A move that changes the place, or the parent within the private
  place, needs `Share` on the page as well as `Write` on the destination;
  reordering among the same siblings needs only `Write`. In the workspace
  place a parent change needs only `Write`, since every member reads every
  workspace page. Left open on purpose: a manager grant on the new ancestor
  makes that member manager of the moved page; trash is recoverable for 30
  days.
- **A departed member.** `pages.forget_member` deletes their grants (unless
  they are a member again); their private pages stay, as does everything
  they made in the workspace place. `pages.share` and the handler take the
  same per-member advisory lock and the handler re-checks the membership, so
  a share racing a departure leaves no grant behind (cloud #291).
