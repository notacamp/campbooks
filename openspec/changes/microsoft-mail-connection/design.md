# Design

## Context

Gmail connects through the front door (ADR 0014): `connectors.start_connection`
checks what connectors owns and asks the identity module's consent door to
start; identity runs the provider round trip (PKCE S256, `state` bound to the
browser, single use, ten minutes) and hands the grant to the kernel's
grant-intake port, which connectors implements: `GrantIntake::adopt` turns it
into a connection through the one seam, `grants::create_or_refresh_connection`.
The port's `mailbox_scopes(provider)` tells identity what a mail connection
asks for, and an empty answer means "not offered": the ways-in listing
(`mailbox_providers`), `start_link` and the sign-in round trip all read it.

Identity already signs people in with Microsoft (`identity::upstream`: the
`CAMPBOOKS_MICROSOFT__*` client, the tenant rule that computes a token's
issuer from its own `tid`, `xms_edov` for a verified address). This change
reuses that round trip and adds the `microsoft` provider to connectors.

## Alternatives considered

### Who knows the mail tenant

The mail connection's tenant may be wider than sign-in's
(`CAMPBOOKS_MICROSOFT__TENANT`): `common` for personal mailboxes where sign-in
is for `organizations`. Identity runs the
consent, so it must send the browser to the mail tenant for a connect or
reconnect; connectors refreshes tokens, so it must refresh there too.

| | Identity reads `CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT` | A port method (`GrantIntake::mailbox_tenant`) | Identity-only `CAMPBOOKS_MICROSOFT__MAIL_TENANT` |
|---|---|---|---|
| One setting | yes | yes | no: connectors needs it to refresh |
| Verifiers built once at start | yes | no: per request, or a cache keyed by tenant | yes |
| Module coupling | reads one key of another module's section (as both already read `CAMPBOOKS_GOOGLE__*`) | widens the kernel port for one provider | none |

**Chosen: one setting, `CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT`, read by
both, defaulting to sign-in's tenant** (`CAMPBOOKS_MICROSOFT__TENANT`, itself
`common` by default). Defaulting to `common` was rejected in the security
review: an app registered for one tenant would break, and one registered for
work accounts would widen without anyone choosing it. An operator who wants
personal mailboxes where sign-in is narrower sets it to `common`. Identity
builds a second Microsoft client for mail round trips at
start-up when the two tenants differ, and uses it for every connect and
reconnect (never for a sign-in or a link). Identity reads the value
leniently: an invalid one is ignored there, and connectors' own check
refuses it only while the flag is on, so the value of a feature that is off
never stops the server.

A connect round trip that should also add a way in ("Also use this account
to sign in") adds one only when sign-in's tenant would admit that account;
otherwise the mailbox connects as a shared one would (no way in), since a
way in sign-in refuses would be a dead entry.

### Mail at sign-in

Google's sign-in asks for mail in the same consent, every time. For Microsoft
that would mint a mail-scoped refresh token at each sign-in of a known
account, which Campbooks drops but cannot revoke (no per-app revoke), and a
work tenant that requires an administrator's consent for `Mail.*` would fail
the sign-in itself. **Chosen: as for Zoho**: a Microsoft sign-in asks for no
mail; only a new account's second round trip (`consent_retry`) asks for the
mailbox, so the first mailbox still comes with sign-up, and connect and
reconnect always ask.

### The connection's account key

| | ID token `sub` | Graph `/me` `id` |
|---|---|---|
| What identity's reconnect check (`expected_subject`) and `connection_of` compare | yes | no: they would never match, so every reconnect would be refused |
| Stable per person and app | yes (pairwise per app; one app serves sign-in and mail) | yes |
| What Graph calls name | no | yes |

**Chosen: both, each where it belongs.** The connection's `account_subject`
stays the ID token's `sub`, the subject identity proved and compares on a
reconnect. The mailbox (the `messages` stream's `mailbox_key`, as Zoho's
`accountId` is) is the Graph user id, with the primary address. They come
from one token response, so they are one account; on a refresh of an
existing connection, a Graph id that differs from the stored mailbox key is
refused (`account_mismatch`) rather than adding a second mailbox, checked in
the transaction that locks the connection.

### The address and whether it is verified

The ID token's `email` claim is not verified by Microsoft (only `xms_edov`
says the domain owner verified it), and the Graph `mail` property of a work
account can be set by that tenant's admin. The connection's address is the
mailbox's own, from Graph (`mail`, else `userPrincipalName`, trimmed and
lower-cased). It counts as verified only when identity found the ID token's
address verified (`xms_edov`) **and** it is the same address: the proof is
used only for the address it proves. Otherwise it is kept, marked
unverified, as for any provider.

### A connection with no sync yet

| | No messages stream until #316 | A messages stream that only refreshes its token |
|---|---|---|
| Health (`needs_reauth`, `throttled`) | nothing runs, so nothing is learnt | the run refreshes the token as it ages |
| Inbox appears | no | yes, empty until #316 |
| What #311 did for IMAP | | this |

**Chosen: the stream, like IMAP before #311.** Its run asks for a valid
access token (stored, or refreshed under the per-connection lock), records a
refusal as `needs_reauth` and throttling as `throttled` through the same
paths as Gmail, and then waits the longest poll interval. It reads nothing,
writes nothing, and its outbound changes are skipped as `unsupported`.

### Granted scopes

Microsoft may answer `scope` with the Graph resource prefix
(`https://graph.microsoft.com/Mail.ReadWrite`) or without it, in any case.
Identity's "mail granted" check and connectors' adoption check both compare
Microsoft scopes without the prefix and ignoring case.

### Revocation

Microsoft's identity platform has no RFC 7009 endpoint, and
`revokeSignInSessions` would sign the person out of every app. Disconnecting
deletes the sealed tokens; nothing is revoked at Microsoft.

## Data ownership and events

Schema `connectors` only. Migration
`20261105010000_connectors__microsoft.sql`: the `connections.provider` CHECK
gains `'microsoft'` (the same lock-timeout pattern as the IMAP migration; no
new table, so no new RLS, grants or foreign keys). Credentials are sealed
with the connectors cipher exactly as Google's.

Events: no new event. `connectors.connection_added`,
`connectors.capability_granted`, `connectors.connection_needs_reauth` and
`connectors.connection_removed` carry `provider: microsoft`; the
`connection-account` v1 and `connection_removed` v1 contracts gain the value
(their enum says more values arrive with their adapters).

## Endpoints and scopes

No new endpoint. `connectors.start_connection` (`connectors:write`) accepts
`provider: microsoft` with `capabilities: ["messages"]` while the provider is
offered (409 `provider_not_configured` otherwise; 400
`capability_not_offered` for any other capability).
`connectors.discover_imap` (`connectors:write`) answers `use_provider:
microsoft` for an Outlook or Microsoft 365 address while Microsoft is
offered, and `not_supported_yet: microsoft` otherwise.

## Settings and boot safety

`CAMPBOOKS_CONNECTORS__MICROSOFT__ENABLED` (default `false`) and
`CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT` (default: sign-in's tenant). Microsoft is
offered only when the flag is on, the `CAMPBOOKS_MICROSOFT__*` client is
configured and the instance secret key is set.

- Flag off: nothing about Microsoft mail is read or checked; the server
  starts whatever the other Microsoft or connectors settings are.
- Flag on, client missing (or no secret key): a warning, not offered.
- Flag on with a tenant that is not `common`, `organizations`, `consumers`
  or a tenant id: the server refuses to start, naming the setting (the only
  combination that stops it).

Production passes an explicit environment list, so a new key never reaches
it until added: the defaults alone must start with production's poll bounds
(`CAMPBOOKS_CONNECTORS__POLL__MIN_SECONDS=60`, `…__MAX_SECONDS=120`), which
the tests pin.

## Security

- `state`, PKCE and the browser binding stay identity's, unchanged.
- Tokens are sealed with the instance cipher (AES-256-GCM, bound to table,
  column and connection), never logged, never in an error (an error carries
  a status and a sanitised code, never a body).
- Token endpoint `https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token`
  and Graph `https://graph.microsoft.com/v1.0/me`: hosts are constants, the
  tenant a validated setting; no redirects; 15-second timeouts; answers above
  64 KiB are refused.
- `/me`: `id` must be 1–64 characters of `[A-Za-z0-9-]`; the address must
  parse as one; the display name is trimmed, control characters dropped,
  at most 256 characters.

## Tracked events

No new tracked event. The plan's `connectors.provider` enum gains
`microsoft` (described "Microsoft: Outlook.com, Hotmail and Microsoft 365
mail"), so the `connectors.connection` and `connectors.send` records (and
every event that carries them: `connectors.connection_added`,
`…_reconnected`, `…_removed`, `…_health_changed`, `…_sync_requested`,
`connectors.send_failed`) can name it. No new operation, job, schedule or
handler, so no new Coverage row.

## Risks

- Personal accounts see an "unverified app" consent until publisher
  verification (#53) is done; documented for operators.
- An instance whose sign-in tenant is narrower than the mail tenant connects
  a personal mailbox without making it a way in, even with "Also use this
  account to sign in" ticked.
