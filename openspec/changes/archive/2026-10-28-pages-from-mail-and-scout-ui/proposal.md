## Why

Part 1 (`2026-10-28-pages-from-mail-and-scout`, cloud #364) gave the server
and Scout what they need to put mail on pages: attachments become a page's
files, a page reads as Markdown, a section can be appended and taken back,
and Scout finds and reads pages and proposes a section or a new page. None of
it shows yet. The client's `RUNNABLE` allowlist does not list the two page
operations, so a page proposal is never offered. This change adds the
screens, so a member can save a thread to a page from the reader and act on
Scout's page proposals.

## What Changes

- **Save to page** in the reader's More menu, with a shortcut. A dialog picks
  an existing page (filtered by name, with the pages already linking the
  thread first) or a new private page, then what to add:
  - a live thread block;
  - a copy of the messages, saying who reads the page;
  - Scout's summary, previewed as a card before anything is added; hidden
    when Scout is unavailable.

  Attachments can be included. An Undo toast follows every save.
- **The page change card** in Scout's chat. It renders the section as the
  page would show it, with source chips, and offers "Add to page", "Edit
  first" and "Discard". It says who will read the page: only you, named
  teammates plus a count of the others, or everyone. It also says when the
  page is kept in step with Notion. `RUNNABLE` gains `pages.append` and
  `pages.create`.
- **Ask Scout on a page.** A page's toolbar (inside its More menu, so it stays
  one row) opens a Scout chat about that page.
- All of it only while the server offers the mail-and-Scout operations
  (`CAMPBOOKS_PAGES__MAIL_SCOUT__ENABLED`), in en, pt-PT, es-ES and fr.

## Capabilities

- `inbox-ui`: Save a thread to a page.
- `assistant-chat-ui`: the page change card.
- `pages-ui`: Ask Scout on a page.

## Impact

- Client only: `clients/app` (reader, pages module, Scout chat) and, where a
  shared piece is needed, `@notacamp/ui`. No server, migration or OpenAPI
  change is expected. If tracked client events are needed, they go in
  tracking patch 0024 (migrations, if any, from 20261120010000).
- Prototype: the "Save a thread to a page" board and the Scout card on the
  "Pages" board of the Pages canvas, approved with change 1.

## Non-goals

- Scout editing or removing existing blocks.
- Saving automatically by rule, and the mobile share sheet.
