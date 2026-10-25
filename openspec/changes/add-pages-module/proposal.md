## Why

Campbooks holds a member's mail, calendar and tasks, but nothing they write
for themselves or their team: the onboarding checklist, the client notes, the
handbook, the meeting notes. Today those live in Notion or Google Docs, away
from the threads, people and tasks they talk about, so every page goes stale
the moment the mail moves on.

The owner asked (2026-10-25) for a built-in "Notion-like thing", mainly for
creating documents and pages: "we're not asking for much different than
notion". Pages are written next to the mail, mention the people, threads,
tasks and events they are about, and are shared with the team the way Notion
shares them.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/363.
Prototype (approved 2026-10-25, "looks good"): https://claude.ai/artifact/B4DKyoW5itjeAhcgAaY2rs

This is the first of three changes in the epic:

1. **This change**: pages, the editor, mentions and backlinks, sharing, trash,
   search.
2. Pages from mail and Scout: "Save to page" from the reader, and Scout reading
   pages and proposing edits.
3. Tables: rows that are pages with typed properties, with table and board
   views.

Modules touched:

- **`pages`**: a new module (crate, `pages` schema, client module). It depends
  on the kernel only, so it needs no `architecture.toml` entry. It is added to
  `docs/domain-map.md` under M2 (*The day*), next to `tasks`.
- **`search`**: indexes pages through the existing `SearchSource` port
  (ADR 0016). No new dependency.
- **Client**: a Pages place, the editor, `@` mentions that render through each
  record's own module.

No cross-module dependency: mentions are stored as `RecordRef`s (ADR 0008),
access to a mentioned record is answered by the kernel's `authz` catalogue
(ADR 0017), and search pulls page text through its port.

## What Changes

- **A page.** It has a title, an optional icon, a parent page (or none), a
  position among its siblings, a place (`private` or `workspace`) and a body.
  The body is a block document: paragraphs, headings (three levels), bulleted
  and numbered lists, to-dos, toggles, quotes, callouts, dividers, code, files
  and images, links to child pages, and mentions.
- **One editor, merged on the server.** The client edits a Yjs document
  through TipTap (ProseMirror). The server keeps the document with `yrs` and
  merges every update, so two people editing the same page never overwrite
  each other. Every update is checked against the block schema and size
  limits before it is stored.
- **Mentions and backlinks.** `@` inserts a person, thread, task, event or
  page as a `RecordRef`, never as a copy of its text. A mention is rendered by
  asking the record's own module, so a reader who cannot open the record sees
  "Something you can't open" and nothing else. Every page shows where it is
  linked from, and `pages.backlinks` lets other places show "Linked from".
- **Sharing like Notion.** Private pages belong to their creator. Workspace
  pages are readable and editable by every member. Any page can also be
  shared with named members as `reader`, `writer` or `manager`. Sub-pages
  follow their parent unless they are shared further.
- **Trash.** Deleting a page moves it and its sub-pages to the trash for 30
  days, with undo, restore, and permanent deletion only from the trash with a
  confirmation.
- **Search.** Page titles and text are found by `⌘K` search, only for callers
  who may read the page.
- **Pages place** (`g p`, new page `n g`), as in the prototype: the tree, the
  breadcrumb, Share, the slash menu (`/`), mentions (`@`), Markdown shortcuts,
  drag to move blocks and pages, the phone layout with a formatting bar, and
  an Undo toast for every page action.
- **Operations** `pages.list`, `pages.get`, `pages.create`, `pages.update`,
  `pages.move`, `pages.sync`, `pages.trash`, `pages.restore`, `pages.purge`,
  `pages.share`, `pages.unshare` and `pages.backlinks`. Scopes are
  `pages:read` and `pages:write`.
- **Off by default.** The place, the operations and the search source are
  behind the `pages` feature flag until the owner turns it on.

## Non-goals

- Saving mail to a page, and Scout reading or writing pages: change 2.
- Tables, properties and views: change 3.
- Live cursors and "who is here", comments, page history beyond undo, page
  templates, public share links, import and export (Notion, Markdown, PDF).
- Editing offline for longer than a lost connection. The Yjs document makes it
  possible later, but this change keeps unsent updates in memory only.
- Any AI call. This change uses no model.
