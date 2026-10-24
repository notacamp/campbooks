# Proposal

## Why

An IMAP mailbox is read (#311), pushed with IDLE (#318) and written back
(#312), but a member cannot send from it: `connectors.send_message` fails
every send from an IMAP inbox with `not_connected`. Replies, forwards and
new mail must leave through the mailbox's own SMTP submission server, with
the same at-most-once guarantee as Gmail and Zoho, and the sent message
must land in the mailbox's Sent folder so every other mail app shows it.

SMTP is older and looser than the providers' APIs: no idempotency key, no
message id in the answer, a server that may or may not keep a copy of what
it sends, limits that differ from provider to provider, and a protocol a
hostile server can stretch without end. Sending has to respect all of that.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/313
(epic https://github.com/notacamp/campbooks-cloud/issues/308).

Modules touched: `connectors` only (the send job's IMAP arm, the SMTP
client, the IMAP stream run's Sent copies, the reader's view of unlocated
sent messages, settings). No new module, no new crate, no new cross-module
dependency, no new endpoint, scope, event, job or schedule. One migration,
`20261102010000_connectors__imap_send.sql`, extends `connectors.sends`
(already under workspace row-level security).

## What Changes

- **Sending over SMTP.** A send from an IMAP inbox goes through the
  connection's SMTP server (`connectors.imap_servers`): port 465 with TLS
  from the first byte, or 587 with STARTTLS required (no STARTTLS: nothing
  more is said, no AUTH is sent). The host is resolved through the kernel's
  guard at every send and only the checked address on a mail port is
  reached; certificates are checked by the platform verifier. It signs in
  (`AUTH PLAIN` or `LOGIN`) with the sealed password, opened for the send
  and dropped after it. Reuses `lettre`, over a connection Campbooks opens
  and bounds itself.
- **From the mailbox only.** The envelope sender and the `From` header are
  the connection's own address, and only once that address is verified (a
  password login proves the username, not an address typed beside it; the
  check for mailboxes whose username differs ships with the connect screen,
  #314); a document whose `From` is any other address is not sent
  (`rejected`). Bcc recipients are in the envelope only: the
  message on the wire has no `Bcc` header; the Sent copy keeps it.
- **Never twice.** The attempt is recorded once the server has accepted
  `DATA` (354) and before the message goes out. A refusal before then, or a
  4xx/5xx answer to the message itself, is definite (retry, or `rejected`).
  A connection lost after the message went out, before the server's final
  answer, is never resent: the send ends `unknown_outcome` at once. A
  `Message-ID` minted per send (the document's) is kept with it.
- **The Sent copy.** After a successful send, the stream's next run (due at
  once), once its read is done, appends the message, `\Seen`, to the SPECIAL-USE `\Sent` folder (or
  the folder the reader found by name), through the session's bounded framer,
  within the server's `APPENDLIMIT` and Campbooks' own cap, unless the SMTP
  host is one of a short list known to keep a copy itself, or the message is
  already in Sent. With UIDPLUS, `APPENDUID` records where it went, so the
  read that follows (and IDLE) emits nothing. An APPEND that fails never
  fails or repeats the send; its code is recorded and logged.
- **The sent message shows at once** in its conversation, indexed and
  announced in the send's transaction as Gmail's and Zoho's are; a sent
  message whose copy could not be made is never removed for having no
  location.
- **Limits.** Per mailbox login (messages an hour, recipients a day), per
  workspace and per SMTP host (messages an hour) budgets in
  `provider_budgets`, a recipients-per-message cap, one SMTP session per login
  at a time, the server's EHLO `SIZE` and Campbooks' own largest message, all
  checked before anything is sent, with conservative defaults and settings.
- **Hostile servers.** Every step has a time limit, what the server sends is
  bounded (the whole session, each reply), and logs carry ids, reply codes and
  counts only, never the server's text, recipients, subjects or bodies.
  Header injection is ruled out by the existing outgoing-document MIME
  builder.
- Still behind `CAMPBOOKS_CONNECTORS__IMAP__ENABLED` (off by default). The
  new settings are checked at startup only while it is on.

## Non-goals

- Proving an unclear send sent or unsent by looking in Sent: a server that
  keeps no copy makes "not found" meaningless, and the reader shows a copy a
  server did keep anyway (see design).
- Send-as aliases or identities other than the connection's address.
- `SMTPUTF8` for non-ASCII mailbox names beyond what the server offers, DSN,
  `REQUIRETLS`, `CHUNKING`/`BDAT`, pipelining.
- OAuth (`XOAUTH2`) to SMTP: IMAP connections sign in with a password.
- Choosing which folder the Sent copy goes to.
