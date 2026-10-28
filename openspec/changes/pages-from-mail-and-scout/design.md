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
