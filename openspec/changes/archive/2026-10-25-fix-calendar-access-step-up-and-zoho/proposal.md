# Calendar access: sign in again when it is needed, and Zoho when the server offers it

Modules: `connectors` (the connection view), `identity` (the reconnect
round trip), and the `schedule` client (Settings → Calendar). Tracking:
https://github.com/notacamp/campbooks-cloud/issues/373.

## Why

In prod, Settings → Calendar → "Add calendar access" on a Google mailbox
answered "Couldn't start that. Try again." The call
(`POST /api/v1/identity/links`, purpose `reconnect`) was refused with 403
`step_up_required`: renewing a grant that still works needs a sign-in from
the last ten minutes. The Calendar settings page turned every refusal into
the same sentence and never offered "Sign in again", which the Mailboxes page
does.

Fixing that exposed two more gaps:

- The reconnect request cannot name `calendar`. The client reached
  `identity.start_link` with only a purpose and a connection, and a reconnect
  asks again for what the connection already has, so even after signing in
  again the consent would not have asked for the calendar scopes.
- The client hard-codes `provider === "google"` for who is offered calendar
  access ("Zoho Calendar is not offered yet"), while the server has a full
  Zoho Calendar adapter behind `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR`. The
  client should read the server's answer.

## What Changes

- `connectors.get_connection` / `list_connections` (and the other places that
  show a connection): the connection view gains `offers`, the capabilities it
  could still be given by consenting again. Google: `calendar`. Zoho:
  `calendar`, only while `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` is on. Microsoft
  and IMAP: none. A capability the connection holds is never in it.
- `identity.start_link` takes optional `capabilities` (`calendar` only), asked
  for besides the mailbox, with what a reconnect already keeps.
- Settings → Calendar lists every connection that offers `calendar` and does
  not hold it, asks for it, and explains a refusal: a step-up gets a banner
  with "Sign in again" that comes back to Settings → Calendar; unsupported,
  rate-limited and failed each get their own short sentence. en, pt-PT, es-ES
  and fr.

## Non-goals

- Turning `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` on in prod, and checking the
  Zoho adapter against a live account (ops, campbooks-cloud#352).
- Offering any capability other than `calendar`.
- Changing the ten-minute step-up rule.

## Impact

OpenAPI: `ConnectionView.offers` (required, additive), `StartLink.capabilities`
(optional, additive). No migration, no scope change, no new event.
