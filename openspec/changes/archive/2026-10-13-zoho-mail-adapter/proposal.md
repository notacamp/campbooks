# Proposal

## Why

Gmail is the only mailbox Campbooks Next can connect. The owner's own mail,
and that of many small businesses Campbooks is for, lives in Zoho Mail, often
several addresses at once. Signing in with Zoho already works (ADR 0014);
the mailbox behind it does not.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/74.

Modules touched: `connectors` (the adapter, the engine's provider dispatch,
the index) and `identity` (Zoho's sign-in asks for mail, and hands the data
centre it vouched for to the grant-intake port). The kernel's
`ProviderGrant` gains one field (`issuer`). No new module, no new
cross-module dependency, no new crate.

## What Changes

- **Zoho Mail REST adapter** in `connectors`: the API of the account's own
  data centre (`mail.zoho.eu`, `.com`, …), derived from the accounts server
  identity validated, never from input.
- **One consent, as with Google**: a Zoho sign-in also asks for
  `ZohoMail.accounts.READ`, `ZohoMail.folders.READ` and
  `ZohoMail.messages.ALL` with offline access; connect and reconnect go
  through identity's front door as for Google.
- **Several mailboxes per connection**: one Zoho login can hold several mail
  accounts; each becomes its own stream and inbox. Several Zoho logins per
  workspace were already possible (one connection per account).
- **Sync by adaptive polling per folder**: Zoho has no push and no history,
  so each run reads the newest page of every tracked folder, and a periodic
  sweep re-reads the window. Messages are fetched as raw MIME, so
  `List-Unsubscribe`, `List-Id`, `Precedence` and `Auto-Submitted` reach the
  decide pass exactly as for Gmail. Zoho's HTML-escaped metadata is decoded
  where it is read.
- **Write-back**: read/unread, star (Zoho flag), archive and unarchive
  (archive, unarchive, or a move back to the Inbox folder), through the
  existing outbound-change records.
- **Send** through Zoho's send and reply API, with the attempt-before-call
  and look-up-before-retry guarantees of #461; the look-up searches the Sent
  folder, since Zoho does not keep a `Message-ID` we set.
- **Rate limits**: a request budget per Zoho account, shared by every
  process, plus Zoho's own 429 throttling.
- The index of emitted messages (`connectors.gmail_messages`) becomes the
  adapters' shared index (`connectors.messages`), with the `Message-ID`.
- Dev-only **fake Zoho Mail** (`pnpm --dir clients fake-zoho`) and an
  in-process fake for the tests.

## Non-goals

- Zoho labels (tags) and Zoho's own sorting are not carried: they need the
  `ZohoMail.tags` scope and have no stream equivalent yet. Label changes and
  moves between streams are recorded `skipped` for Zoho.
- IMAP/SMTP access, push (Zoho has none for Mail), Zoho Calendar or WorkDrive.
- An "add a mailbox" screen: connecting another mailbox stays an API
  operation (`connectors.start_connection`) plus the first-run step.
- Moving mail to an arbitrary folder: the outbound-change contract has no
  such intent; unarchive moves a message back to the Inbox folder.
