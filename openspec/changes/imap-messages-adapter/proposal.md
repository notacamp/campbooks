# Proposal

## Why

`imap-connection` (#310) lets a member connect any IMAP mailbox, but nothing
reads it yet. This change is the reading half: an IMAP mailbox fills the
inbox, Sent, Spam and Trash exactly as a Gmail or Zoho mailbox does, from the
same `MessageChangeBatch` v1 stream, so `conversations` and everything above
it need no change.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/311
(epic https://github.com/notacamp/campbooks-cloud/issues/308).

Modules touched: `connectors` only (the IMAP adapter and its index of
message locations). No new module, no new crate (uses `async-imap` from
#310).

## What Changes

- **Folders become mailbox places**: SPECIAL-USE attributes (RFC 6154) mark
  Inbox, Sent, Junk, Trash and Archive; servers without them are matched by
  well-known folder names in the languages Campbooks ships (and common
  provider names such as Yahoo's `Bulk`). User folders are read too and
  appear as labels. Drafts, `\All` and `\Flagged` virtual folders and
  unselectable folders are never read.
- **Backfill, then incremental, then sweeps**: a new mailbox reads the
  configured window (default 90 days) newest first within the run's byte
  budget; each later run asks every folder for its `STATUS` and opens only
  folders whose `UIDNEXT`, `HIGHESTMODSEQ` or message count moved; a
  periodic sweep re-reads the window's flags and finds messages deleted or
  moved without CONDSTORE.
- **One message, many places**: a message is identified by its normalised
  `Message-ID` (a stable digest of its headers when it has none), so a move
  between folders keeps the same message and a copy in two folders is one
  message. Each location (folder, UIDVALIDITY, UID) is kept in the index for
  write-back (#312).
- **Reading never marks mail read**: bodies are fetched with `BODY.PEEK[]`.
- **UIDVALIDITY changes re-read that folder** without duplicating messages.
- **Health**: repeated login refusals at sync set `needs_reauth`; the
  server's own "unavailable" / "in use" answers throttle instead.
- **Per-host session cap** so a big provider never sees more than a few
  Campbooks sessions at once.
- Still behind `CAMPBOOKS_CONNECTORS__IMAP__ENABLED` (off); the connect
  screen (#314) turns it on.

## Non-goals

- Push with IDLE (#318); this change polls adaptively, as Zoho does.
- Write-back (#312) and send (#313).
- Mirroring streams into IMAP folders (#312).
- IMAP keywords (`$Label1`…) as labels.
