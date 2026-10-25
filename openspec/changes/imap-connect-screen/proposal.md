# Proposal

## Why

The IMAP adapter can find a mailbox's servers (#310), read it (#311), push
with IDLE (#318), write back (#312) and send through its SMTP server (#313),
but no member can reach any of it: the Mailboxes screen offers only the
OAuth providers, and IMAP is off by default. This change is the last slice of
the IMAP epic: the screen that connects any mailbox with its address and an
app password, the automatic check of an address the login does not prove,
and IMAP turned on.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/314
(epic https://github.com/notacamp/campbooks-cloud/issues/308). It also
corrects the stale Zoho notes of https://github.com/notacamp/campbooks-cloud/issues/334
(the docs part).

Modules touched: `connectors` (server: the address check, its job and its
retry operation, the connection view, the IMAP default; client: the
Mailboxes screen's connect flow). No new module, no new crate, no new
cross-module dependency. One migration,
`20261104010000_connectors__address_verification.sql`, in schema
`connectors`.

## What Changes

- **Add a mailbox, by address first.** The Mailboxes screen's Connect
  section becomes one "Add a mailbox" button (⇧C) that opens a dialog with a
  single address field. `connectors.discover_imap` then picks the route:
  Gmail / Google Workspace and Zoho go to their existing sign-in buttons;
  Outlook, Hotmail, Live and Microsoft 365 get a plain "coming soon" note
  (Microsoft Graph is #309); every other address gets the app-password step.
- **The app-password step** shows a short how-to for the provider (Yahoo,
  AOL, iCloud, Fastmail, GMX and web.de with "turn on IMAP first", Yandex,
  and a generic text for a custom domain), each linking to the provider's
  own help page; one password field; and "Advanced" (collapsed, prefilled
  from discovery): username, IMAP and SMTP host, port and security, TLS or
  STARTTLS only.
- **Errors in plain words**, from the backend's stable codes, never the
  server's text; the step-up refusal offers "Sign in again" as the screen
  already does. The password is never logged and never kept in client state
  after submit.
- **Address check.** A mailbox whose username is not its address
  (`address_verified` false) is checked right after it connects: Campbooks
  sends a one-time code to the address through the mailbox's own SMTP server
  and looks for it in that mailbox's Inbox over IMAP, in the stream's own
  runs. Found: the address is verified and sending works. The member types
  nothing. The screen shows "Checking your address…", then "Verified" or
  "We couldn't verify this address: sending stays off", with a retry
  (`connectors.check_address`). The check message is removed afterwards
  (UIDPLUS-guarded `UID EXPUNGE` of exactly its UID, else a move to Trash,
  else left in place).
- **IMAP on by default.** `CAMPBOOKS_CONNECTORS__IMAP__ENABLED` defaults to
  true. An instance without the instance secret key still starts (IMAP is
  then simply not offered); only an explicit `…__ENABLED=true` without the
  key stops it, as before. IDLE stays off. Every startup check passes on
  production's settings (poll bounds 60 to 120 seconds, no IMAP variable
  set).
- **Docs.** `deploy/.env.example`, `deploy/README.md` and the connectors
  spec say IMAP is on; the Zoho notes list the four mail scopes Campbooks
  asks for (`ZohoMail.tags.ALL` included) and say labels are synced.

## Non-goals

- Microsoft Graph (#309): Outlook addresses get a note only.
- Proving an address on a server whose operator is the one connecting it
  (see design: such a server sees the code it relays; the check defends the
  honest-but-lax relay and the lying answer).
- Send-as aliases; editing a connected mailbox's servers (reconnect does
  it); a first-run (onboarding) path for IMAP.
- Removing the check's copy in Sent at servers that keep one.
- Tracked events: the connectors module is not instrumented yet
  (campbooks-cloud#332); no Coverage row is added.
