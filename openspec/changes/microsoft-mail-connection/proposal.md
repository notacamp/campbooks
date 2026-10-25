# Proposal

## Why

Microsoft turned off password (basic) authentication for Outlook.com in
September 2024, so Outlook.com, Hotmail, Live and MSN mailboxes cannot go
through the IMAP adapter with an app password, and neither can most
Microsoft 365 tenants. They need OAuth and Microsoft Graph. Today "Add a
mailbox" tells everyone with such an address that Outlook is coming soon.

This change is the first slice of the Microsoft Graph epic: **connecting** a
Microsoft mailbox through the connectors front door, the way Gmail connects.
Reading (#316) and write-back and send (#317) follow as their own changes.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/315
(epic https://github.com/notacamp/campbooks-cloud/issues/309).

Modules touched: `connectors` (the new provider, its OAuth client for token
refresh and Graph `/me`, the grant intake, health, discovery routing, the
connect screen), `identity` (the consent round trip of a mail connection
goes to the mail tenant, and Microsoft's granted scopes are compared without
their Graph prefix). No new module, no new cross-module dependency, no new
crate.

## What Changes

- **A fourth provider, `microsoft`**, for the `messages` capability. One
  consent asks for `offline_access Mail.ReadWrite Mail.Send User.Read`
  (with `openid email profile`, as every round trip does), at tenant
  **`common`** by default, so personal Microsoft accounts (Outlook.com,
  Hotmail, Live, MSN) connect as well as work and school ones. The tenant is
  `CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT`, separate from sign-in's
  `CAMPBOOKS_MICROSOFT__TENANT`.
- **The same OAuth app as Microsoft sign-in** (`CAMPBOOKS_MICROSOFT__CLIENT_ID`
  and its secret), as Google and Zoho share theirs.
- **The same front-door rules as Gmail**: a connection is its grantor's; the
  same account connected again refreshes it; a reconnect with another
  account is refused (`account_mismatch`); "Also use this account to sign
  in"; a Microsoft sign-in's grant is adopted when its scopes allow.
- **The mailbox is read from Graph**: after the code exchange, Graph `/me`
  gives the mailbox key (the Graph user id) and its address (`mail`, else
  `userPrincipalName`, normalised). Everything Graph returns is validated,
  answers are size-bounded and calls time out. Calls go only to
  `login.microsoftonline.com` and `graph.microsoft.com`.
- **Health**: a refresh that Microsoft refuses because the grant is gone
  (`invalid_grant`, `interaction_required`, and AADSTS errors such as 70000,
  700082, 50173 and 65001) marks the connection `needs_reauth`; 429 and
  `Retry-After` mark it `throttled`.
- **No sync yet**: the connection's messages stream exists and refreshes its
  token, but reads nothing until #316, so it never fails.
- **Off by default**: `CAMPBOOKS_CONNECTORS__MICROSOFT__ENABLED` (default
  `false`). While off, Microsoft is not offered as a mail connection even
  where the Microsoft sign-in client is configured, and the connect screen
  keeps "Outlook is coming soon". A setting of a feature that is off never
  stops the server.
- **Connect screen**: when the instance offers Microsoft, an Outlook,
  Hotmail, Live, MSN or Microsoft 365 address goes to Microsoft's own sign-in
  ("Connect Outlook", with "Also use this account to sign in"), like Gmail.

## Non-goals

- Reading mail, folders, delta sync and push (#316).
- Write-back, categories and sending (#317). A Microsoft connection sends
  nothing: a send through it ends `not_connected`.
- Revoking a Microsoft grant at disconnect: Microsoft has no endpoint that
  revokes one app's refresh token without signing the person out of every
  app; the tokens are deleted with the connection, and the person removes
  the app at <https://account.live.com/consent/Manage> or
  <https://myapps.microsoft.com>.
- Microsoft calendar.
- Publisher verification (#53): an operator task, documented.
