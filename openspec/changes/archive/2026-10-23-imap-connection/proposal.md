# Proposal

## Why

Campbooks Next connects Gmail and Zoho Mail only. Most other mailboxes
(Yahoo, AOL, iCloud, Fastmail, GMX, web.de, Yandex, Mail.ru, and every
custom domain on a hosting company's mail server) offer no API Campbooks can
use, but all of them speak IMAP and SMTP with an **app password**. One
generic IMAP + SMTP adapter covers them all (ADR 0010 already lists IMAP as a
mail adapter). This change is its first slice: **connecting** such a
mailbox. Reading (#311), write-back (#312), send (#313) and the connect
screen (#314) follow, each as its own change.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/310
(epic https://github.com/notacamp/campbooks-cloud/issues/308).

Modules touched: `connectors` (the new provider, settings discovery, the
verify step, the password credential, the endpoints) and `kernel` (the SSRF
host guard and the step-up rule move here from `identity`, which keeps using
them unchanged). No new module, no new cross-module dependency. New crates:
`async-imap` (IMAP client) and `hickory-resolver` (SRV and MX lookups);
`lettre` (SMTP), `tokio-rustls` and `quick-xml` are already in the
workspace.

## What Changes

- **A third provider, `imap`**, beside `google` and `zoho`. A connection is
  one mailbox: an address, a username, an IMAP server and an SMTP server.
- **Settings found from the address**: a bundled copy of Thunderbird's
  provider database, then the domain's own autoconfig document, then DNS SRV
  records (RFC 6186), then the MX host. Addresses that belong to a provider
  Campbooks connects natively (Gmail and Google Workspace, Zoho, and later
  Outlook / Microsoft 365) are sent to that provider's sign-in instead.
- **Verified before saved**: Campbooks signs in to the IMAP server and
  authenticates to the SMTP server (without sending) before it keeps the
  connection. A refusal says what went wrong in plain words: wrong password,
  an app password is needed, the server cannot be reached, or its
  certificate is not valid.
- **The password is a credential like a token**: sealed with the connectors
  key, bound to its row, never returned by any operation.
- **TLS only, public hosts only**: implicit TLS or STARTTLS, certificates
  verified; on the hosted service, hosts that resolve to private, loopback,
  link-local or metadata addresses are refused at every connection.
- **Hard to misuse as a password-guessing service**: attempts are limited
  per member, per workspace and per target mailbox.
- **Off by default** (`CAMPBOOKS_CONNECTORS__IMAP__ENABLED`) until reading lands in #311;
  while off, `imap` is not offered and its endpoints answer
  `provider_not_configured`.

## Non-goals

- Reading, write-back and sending (#311, #312, #313); the connect screen
  (#314).
- POP3 (decided 2026-10-23: no folders, read state or Sent, so the two-way
  features would silently break).
- Proton Mail (needs Proton Bridge; parked).
- OAuth for IMAP (XOAUTH2): providers with OAuth are connected through
  their own adapters (Google, Zoho, Microsoft #309).
- Signing in with an IMAP mailbox: a password login proves control of a
  mailbox, not an identity, so an IMAP connection is never a way in.
