# Calendar access: say why it came back refused, and make Zoho show its screen

Modules: `schedule` client (Settings → Calendar), `identity` (the provider
client). Tracking: https://github.com/notacamp/campbooks-cloud/issues/381.

## Why

Adding calendar access to a Zoho mailbox failed three times with no message.
Zoho answered at once as the Zoho account the browser was already signed in to,
ignoring `login_hint`; that was not the mailbox's account, so the callback
refused it (`account_mismatch`) and sent the member back to Settings → Calendar
with `?link_error=account_mismatch`. That page ignored the query. A member with
several Zoho logins hits this easily.

## What Changes

- Settings → Calendar reads `?link_error=` and `?connected=` once (the
  Mailboxes page's parsing, `mailboxResult`), clears them, and says what
  happened: for `account_mismatch`, "<Provider> answered as another account.
  Switch to <address> at <Provider>, then try again."; for a success, "Calendar
  access added for <address>."; a cancelled or any other refusal each get a
  short calendar-worded sentence. en, pt-PT, es-ES, fr.
- A Zoho authorization request for a mailbox (connect, reconnect: offline
  access) always carries `prompt=consent`, so Zoho shows its screen and the
  member sees which account answers. Zoho has no `select_account`. Google and
  Microsoft are unchanged.

## Non-goals

- Changing which account Zoho answers as, or how the callback checks it.
- Mailboxes page copy; it already explains these refusals.

## Impact

No API, migration, scope or event change.
