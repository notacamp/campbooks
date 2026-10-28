# Design

## Alternatives considered

**Where a copy of the messages is built.**

- *In the client* (chosen). The reader already holds the messages the member
  may read, sanitised for display. Converting them through the editor's paste
  sanitiser and syncing them as an ordinary edit needs no new server path into
  conversation content, and it cannot reveal more than the member already saw.
- *A server operation that reads the conversation.* That would need a new
  cross-module read of message bodies, a second sanitiser, and a new
  content-bearing path. Rejected.

**How Scout's summary is made.**

- *A Scout chat about the conversation that proposes a page change* (chosen).
  It reuses the chat (`complete`), its about check, its limits, its logging
  rules and its proposal cards. The summary is a proposal, so it is seen
  before it is added and can be undone. This is not the per-message read
  (`conversations.read_message`): it is a chat the member started, like any
  "Ask Scout".
- *A dedicated "summarise thread" operation.* A second model path over mail
  content, with its own prompt, limits and logging. Rejected.

**Attachments.**

- *Attach the existing blob to the page* (chosen). Blobs are
  content-addressed per workspace (`kernel::blobs`), so a page holds the same
  bytes by attaching its own owner. `conversations` answers the blob through a
  kernel port only after its own read check, and `pages` never learns more
  than the file's name, type, size and blob.
- *Download and re-upload from the client.* That doubles the traffic, is
  limited by the browser, and stores nothing new anyway. Rejected.

## The `AttachmentBlobs` port (kernel)

`fn blob_of(tx, access, conversation, message, attachment) ->
Option<AttachmentBlob { blob: BlobRef, filename, content_type, size }>`.

`conversations` implements it. It answers `None` unless `access` may read the
conversation (the same `authz` check as `conversations.get_attachment`), and
the attachment belongs to that message of that conversation. It is
registered in `AppState` like the other ports. Without a registered
implementation, `pages.attach_from_message` answers 404 for everything.

## Operations (module `pages`)

| Operation | Scope | Does | Undo |
| --- | --- | --- | --- |
| `pages.attach_from_message` | `pages:write` | Body `{ page, conversation, message, attachments: [id] (1–20) }`. Needs `write` on the page; each attachment goes through the port (404 if any is `None`). Inserts `pages.files` rows (`uploaded_by` = caller, `source` = the attachment's `RecordRef`), attaches owner `pages.page`, and answers the file descriptors for the client's file blocks. Counts against the page's 20 MiB-per-file rule, not a byte copy. | `pages.detach_files` |
| `pages.detach_files` | `pages:write` | Body `{ page, files: [id] (1–20) }`: removes those `pages.files` rows and their blob attachments when no block on the page still names them. Others stay, and the answer says which. | `pages.attach_from_message` with the same input |
| `pages.read_text` | `pages:read` | The page as Markdown: the title as `#`, blocks in order, to-dos as `- [ ]`/`- [x]`, mentions and embeds as `[type:id]` (never resolved), files as their names. Capped at 64 KiB, cut at a block boundary with `truncated: true`. Opted in with `.assistant()`. | — |
| `pages.append` | `pages:write` | Body `{ page, markdown (≤ 16 KiB), sources: [RecordRef] (≤ 10) }`. Parses the Markdown into schema blocks on the server (headings, paragraphs, lists, to-dos, quotes, code, links with allowed schemes; anything else becomes plain text) and applies them as a Yjs update at the end of the page, through the same checker as `pages.sync`. It records the inserted blocks' ids as an `append_id`. | `pages.unappend` |
| `pages.unappend` | `pages:write` | Removes the blocks of an `append_id` that are still on the page. Blocks someone has edited since stay, and the answer says how many stayed. | `pages.append` with the same input |
| `pages.list` | — | Gains `q` (title and text search through `search`), used by `find_pages`. Opted in with `.assistant()`. | — |

`pages.create` gains `markdown` (the same parser), used by Scout's "new page"
proposal. `pages.attach_from_message`, `pages.append` and `pages.create` are
not opted in for direct assistant calls: Scout can only *propose* them.

## Scout (module `assistant`)

- **Tools** (`ChatTool` variants in `features/chat/tools.rs`, mapped in
  `OPERATIONS`):
  - `find_pages { q?, limit ≤ 10 }` → `pages.list`: id, title, place, edited
    at. No text.
  - `read_page { page }` → `pages.read_text`: bounded by `MAX_RESULT_BYTES`
    (8 KiB) through `project::bounded`.
- **About registry**: `pages.page` → `pages.read_text`, capped by
  `MAX_ABOUT_BYTES`. The client's `SCOUT_CHAT_ABOUT` gains `pages.page`, and
  "Ask Scout" on a page opens a chat about it.
- **Proposal** `propose_page_change` (a `Proposing` tool, counting toward
  `MAX_PROPOSALS`), with two actions mapped in `ROWS`:
  - `append_section { page, markdown, sources }` → `pages.append`, undo
    `pages.unappend`.
  - `new_page { title, place: private, parent?, markdown, sources }` →
    `pages.create`, undo `pages.trash`. Scout always proposes new pages as
    private; the member moves them to share.
  The proposal is checked when stored: the page is writable by the member,
  the Markdown parses within the caps, and every source is readable by the
  member. The client's `RUNNABLE` allowlist gains exactly these two
  operations, and a `PageChangeCard` renders the section as the page would,
  with source chips, "Add to page" / "Edit first" / "Discard", as in the
  prototype.
- **Handbook**: `crates/modules/pages/handbook.md` (≤ 8 KiB), composed in
  `crates/server/src/lib.rs`:
  - when to read a page;
  - that pages are the member's own words, and Scout quotes them rather than
    paraphrasing;
  - that Scout only adds and never rewrites;
  - that it cites the threads it used as sources;
  - that a summary for a workspace page will be read by the whole team.
- **AI**: role `complete` (the chat). When `complete` is not configured, Scout
  is unavailable, as everywhere: the "Scout's summary" choice and "Ask Scout"
  on a page are hidden, while the live block, the copy and attachments still
  work. Undo is `pages.unappend` or `pages.trash`, from the card's Undo or
  the page.
- **Logging**: as for every chat, sizes and names only. No page text, title
  or Markdown goes into logs, events or tracked events.

## The reader (client)

- `ConversationReader.tsx` gains a "Save to page" `OverflowAction` beside
  "Make it a task", with a shortcut registered in the shortcut catalogue
  (`pages.save_thread`; the prototype's `⇧P` if it is free, else the next
  free key). It opens `SaveToPageDialog`, exported from `~/modules/pages`'s
  `index.ts`, as `MakeTaskForm` is from tasks.
- **Dialog** as in the prototype:
  - "Add to a page" or "New page";
  - a name filter, with the pages that already link the thread first (from
    `pages.backlinks`);
  - the three choices, the attachments checkbox, and Save with `⌘↵`.
  "Scout's summary" turns Save into "Preview summary", which opens the Scout
  panel with the proposal.
- **Copy**: each message becomes a heading-3 line ("Sender — date"),
  followed by its sanitised body, inserted at the end of the page in one
  sync. The quoted history of each message is dropped, as the reader does.
- **Undo toast**: the client knows what it inserted and removes it (Yjs);
  attachments are detached with `pages.detach_files`.

## Access

- `pages.attach_from_message` needs `write` on the page and the port's read
  answer for every attachment. A missing or unreadable one makes the whole
  call 404, and nothing is attached.
- A copy or summary puts mail text into the page. From then on it is page
  content, readable by whoever may read the page. The dialog and the
  handbook say so. This is the member's choice, as in any editor.
- Scout's tools run as the member (`Member::read`, `Caller::Assistant`), so
  every page they see is one the member may read. `pages.read_text` never
  resolves mentions, so Scout learns no more about a mentioned record than
  its type and id unless it reads that record through its own tool.

## Events and tracking

- `pages.page_changed.v1` covers append, unappend and attach as `body`
  changes (no new event).
- Tracked events:
  - `thread_saved_to_page` (`what`: `live` | `copy` | `summary`,
    `attachments`: count, `target`: `existing` | `new`);
  - `page_proposal_settled` (`action`: `append_section` | `new_page`,
    `outcome`: `accepted` | `discarded` | `undone`).
- Coverage rows for the four new operations and the port.

## Deviations while building (part 1)

The design was written before pages shipped (#672, #680, #681, #685). Checked
against the code on `next` (task 0.3), what differs:

- **Routes carry the page in the path**, as every pages operation does:
  `POST …/pages/{page_id}/append`, `…/unappend`, `…/files/from-message`,
  `…/files/detach` and `GET …/pages/{page_id}/text`. The bodies drop `page`.
- **A file has no id of its own** (`pages.files` is keyed by page and blob
  hash), so `pages.detach_files` takes blob hashes and answers
  `{ removed, kept }`. It removes only files that came from a message (a new
  `source_type`/`source_id` on `pages.files`, migration `20261115020000`) and
  that no block names; a file the member uploaded is never touched.
- **`AttachmentBlobs`** answers `{ blob, filename }` (the type and size are on
  the `BlobRef`), takes `&AppState` and plain `Uuid`s, and is implemented by
  `conversations` with the same catalogue `read` on `conversations.attachment`
  and inbox membership as `conversations.get_attachment`, then the same join on
  message and conversation. `pages.attach_from_message` also refuses a table,
  a trashed page and a file over 20 MiB (413 `file_too_large`).
- **`attach_from_message` announces nothing**: it changes no block. The file
  blocks reach the page through `pages.sync`, which announces the body change.
- **`pages.list` gains no `q`.** The search index already holds pages (ADR
  0016) and answers only what the caller may read, so `find_pages` asks
  `search.query` with `types=pages.page` (a new limit on record types, so mail
  hits cannot crowd pages out of the result) and reads titles, places and
  dates from `pages.list` (tables left out). `pages.list` is opted in with
  `.assistant()`.
- **`.assistant()` on `pages.append`, `pages.unappend`, `pages.create` and
  `pages.trash`.** The catalogue's convention, checked by the assistant's tests,
  is that an operation a proposal may build carries `.assistant()` (Low risk).
  The server still never runs one for a chat: `invoke` refuses a mutating
  operation without an idempotency key, and `Member::read` carries none.
- **`pages.append`'s `sources`** end the section as a paragraph of mentions, so
  they are links and backlinks. The pages module checks them structurally (a
  record type and id a page may point at, at most ten); that the member can read
  each one is checked by Scout when the proposal is stored. `pages.create` takes
  `markdown` and `sources` too, off while the flag is off (404).
- **`pages.unappend` is idempotent**: an `append_id` that is unknown, belongs to
  another page or was taken back already answers 200 with nothing removed. An
  append is remembered in `pages.appends` as block ids and content hashes (no
  text). A list item is the unit inside a list.
- **Markdown** is a small fixed subset read line by line (`domain::markdown`);
  lists are one level (indentation is read but not kept), and an image, a table
  or a footnote is plain text. Blocks are made with the Notion block builder, so
  a synced page sends an append like any edit.
- **From the security review:**
  - `pages.attach_from_message` needs `conversations:read` as well as
    `pages:write`. The attachments port checks the member, not the token.
  - Bytes the store no longer holds are a 404.
  - The Markdown reader pairs brackets and parentheses in one pass, so its
    work is linear in the text (a step-counted test).
  - `pages.unappend` keeps the append, trimmed to the blocks still there,
    while edited blocks remain, so a second undo can still remove them.
  - The `blocks` count leaves out the sources line.
- **`pages.get_sharing`** (new, read, assistant opt-in, behind the flag)
  answers who really reads a page: everyone for a workspace page; otherwise
  the creators up the chain and every grant on the chain. It also says
  whether a Notion link is syncing. Scout's page proposal carries this as
  `audience` (`only_you`, `named` or `everyone`), `readers`, `others` and
  `notion`; a new page takes its parent's readers. The card never says
  "private" when anyone else can read the page, and `detail.markdown`
  carries the full Markdown for part 2's card.
- **`pages.read_text`** answers `{ markdown, truncated }` and 409 for a page in
  the trash. `find_pages` and `read_page` add no operation of their own.
- **Scout's proposal** keeps its page facts in `ChatProposal.page` and the
  Markdown and sources in `detail`; `new_page`'s `parent_id` must be a private
  page the member can write. Store-time caps are the 16 KiB of Markdown and the
  ten sources; the page parser itself runs when the member confirms.
- **The flag reaches the assistant** as `AssistantDeps.pages`, set from the
  pages module's switch by the server, which composes the pages handbook section
  only while it is on.
- **Tracked events**: this part tracks `pages.files_attached` and
  `pages.section_appended` (patch 0021), and adds `find_pages`, `read_page`,
  `append_section` and `new_page` to the assistant's tool and action enums, so
  settling a page card is tracked by the existing proposal event.
  `thread_saved_to_page` and `page_proposal_settled` are the client's, in part
  2. The port is not a Coverage subject: Coverage rows are the five operations.
