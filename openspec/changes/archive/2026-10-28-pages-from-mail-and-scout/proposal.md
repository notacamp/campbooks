## Why

Pages (change 1, `add-pages-module`) let a member write next to their mail,
but getting mail *into* a page still means copy and paste, and Scout cannot
see or help with what the member writes. The owner asked for both: "Save mail
to a page" and "Scout reads & writes" (2026-10-25). The approved prototype
shows them as the "Save a thread to a page" board and the Scout panel on a
page.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/364
(epic #363). Prototype: https://claude.ai/artifact/B4DKyoW5itjeAhcgAaY2rs

Modules touched:

- **`pages`**: three operations, `pages.attach_from_message`,
  `pages.read_text` and `pages.append` (with its undo `pages.unappend`), plus
  its handbook section.
- **`conversations`**: implements a new kernel port,
  `AttachmentBlobs`, that answers an attachment's blob for a caller who may
  read it. No module depends on another.
- **`assistant`**: the tools `find_pages` and `read_page`, the proposal
  `propose_page_change`, and `pages.page` in the about registry.
- **Client**: nothing in this part. Part 2 adds the reader dialog, the page's "Ask Scout" and the proposal card.

## Delivered in two changes

The owner signed off ("go ahead", 2026-10-28) and asked for smaller steps.
**This change is part 1:** the server side and Scout, behind
`CAMPBOOKS_PAGES__MAIL_SCOUT__ENABLED`, off by default. That covers the
`AttachmentBlobs` port, the pages operations (attach and detach files, read
as text, append and unappend), Scout's tools `find_pages` and `read_page`,
the `pages.page` about-anchor, and the `propose_page_change` proposal.
**Part 2** (`pages-from-mail-and-scout-ui`, its own PR) adds the reader's
"Save to page" dialog, the copy of messages, the summary hand-off, the page's
"Ask Scout", and the client's `PageChangeCard`, plus its `RUNNABLE` entries.
Until part 2 ships, the client runs none of these proposals, because its
`RUNNABLE` allowlist doesn't list them.

## What Changes

- **Save to page** in the reader's More menu, with a shortcut. A dialog picks
  an existing page (filtered by name; the pages already linked to the thread
  come first) or makes a new private one, then picks what to add:
  - **A live thread block**: an `embed` of the conversation, as in change 1.
    It needs no server call beyond `pages.sync`.
  - **A copy of the messages**: the client turns the messages it already
    shows into page blocks, through the same sanitiser as paste. The dialog
    says that everyone the page is shared with will read the copy.
  - **Scout's summary**: a Scout chat about the conversation proposes a
    section, and the member sees it before it is added (below).
  - **Attachments too** (optional): `pages.attach_from_message` makes the
    thread's files the page's files without copying bytes, and the client
    adds file blocks for them.
  An Undo toast follows every save.
- **Scout reads pages.** `find_pages` (by title and text, readable pages
  only) and `read_page` (the page as plain Markdown, mentions as typed
  references) let Scout answer from what the member wrote. A chat can be
  about a page: "Ask Scout" on a page opens one.
- **Scout proposes page edits.** `propose_page_change` proposes either a new
  section at the end of a page (`pages.append`) or a new page
  (`pages.create`), in Markdown, with its sources. Like every Scout change it
  runs only when the member confirms the card, and it can be undone.

## Non-goals

- Scout editing or deleting existing blocks: it only adds. Rewriting a
  member's text can come later, with a diff view.
- Automatic saving (a rule that files every invoice thread into a page).
- Tables (change 3) and Scout filling table properties.
- Saving mail into pages from the mobile share sheet.
