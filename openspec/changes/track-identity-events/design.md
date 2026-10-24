# Design

How to add a tracked event is in `docs/tracking.md`; the decision is
ADR 0020. This change applies it to the whole `identity` module.

## Data ownership and events

Every snapshot is read from `identity`'s own rows, in the transaction of the
change, after it: `identity.accounts`, `.sessions`, `.provider_identities`,
`.passkeys`, `.clients` and `.consents`. New queries are runtime
`sqlx::query_as`. Events go to `kernel.tracked_events` (the kernel's
outbox); nothing else is written. No domain event is published or consumed
that was not before, no endpoint, scope or AI role changes.

What an event may hold: ids, enums, booleans, counts and timestamps. Never
an address (typed, verified or held), a display or passkey name, a provider
subject or issuer, a token, a secret, a code, a challenge, a credential id,
an authenticator model (AAGUID), a user agent, a redirect URI, a scope's
name, an IP address or a free-text error. A refusal is an error *kind*
(`identity.sign_in_failure`).

**Client ids.** A client is named by its `client_id` only when it is one
Campbooks minted (`campbooks-web`, `campbooks-native`, `cb_…` for a
workspace's app, `dcr_…` for a registered one), as a `code` with that
pattern. A client known by its metadata document has a URL for an id: the
property is left out and only its kind (`metadata_document`) is kept.

**Scopes.** Instance-wide events (accounts, sessions, ways in, passkeys,
consents, tokens) are written from the module's instance transactions.
Workspace apps live in their workspace (`begin_for`), so their events are
workspace-scoped. The tracker refuses an instance-wide event from a
transaction that works in a workspace, so the two handlers delivered in a
workspace (a connection removed, a member left) track workspace-scoped
events of their own (`connected_way_in_removed`, `consents_revoked`).

## Actors

The HTTP layer opens every request's context as `anonymous`, and only
`Scoped` (a bearer token) and `Admin` name the caller. The issuer's
protocol and sign-in endpoints authenticate with cookies, client
credentials or nothing, so their events come out as **`anonymous`**, with
the account in the body (`account_id`, or the snapshot's):

- `sign_in_succeeded`, `sign_in_failed`, `account_created` (provider
  callback, new-account screen, passkey sign-in and sign-up);
- `way_in_added`, and `way_in_removed` with cause `connect_failed` (the
  provider callback of a signed-in round trip);
- `passkey_counter_regressed`;
- `consent_granted`, `consent_denied` (the consent screen);
- `session_ended` (logout with an ID token hint);
- `tokens_issued`, `refresh_reuse_detected`, `refresh_token_revoked` (the
  token and revocation endpoints);
- `dynamic_client_registered` (`/oauth2/register`).

The API operations (`/api/v1/identity/*`) are `user`; the admin operations
`admin`; the handlers and the prune schedule `system`, on behalf of whoever
caused them. The mailbox hand-over of a round trip runs in a spawned task,
which keeps the request's context (`tracking::scope`), so what it tracks
(and what the connectors module tracks for it) is still the request's.

## Records (snapshots)

| Record | Fields |
| --- | --- |
| `identity.account` | `id`, `suspended`, `provider_ways_in`, `passkeys`, `created_at` |
| `identity.session` | `id`, `account_id`, `method` (`provider`, `passkey`), `provider`?, `auth_time`, `expires_at`, `ended_at`? |
| `identity.way_in` | `id`, `account_id`, `provider`, `linked_via` (`sign_up`, `link`, `connect`), `address_verified`, `linked_at`, `last_used_at`? |
| `identity.passkey` | `id`, `account_id`, `added_via` (`sign_up`, `passkey`), `backup_eligible`, `backup_state`, `created_at`, `last_used_at`? |
| `identity.client` | `client_id`?, `kind` (`first_party`, `workspace`, `dynamic`, `metadata_document`), `confidential`, `scope_count`, `redirect_uri_count`, `created_at`, `secret_rotated_at`? |
| `identity.consent` | `id`, `account_id`, `client_id`?, `client_kind`, `bound_workspace_id`?, `scope_count`, `remembered`, `granted_at`, `revoked_at`? |

`?` marks an optional field. `account_id` and `bound_workspace_id` are
`pseudonymous`; the records' own ids and `client_id` are `id`.

## Events

All are version 1, `live`, added in plan version 3 by this change.

### Signing in (instance, `anonymous`)

| Event | Fires when | Properties |
| --- | --- | --- |
| `sign_in_succeeded` | an issuer session starts: a known provider identity at the callback, `create_account`, `finish_passkey_sign_in`, `finish_passkey_sign_up` | `session`, `new_account` |
| `sign_in_failed` | a sign-in is refused: at the provider callback (`invalid_state`, `cancelled_at_provider`, `provider_unavailable`, `login_failed`, `sign_in_not_allowed`, `account_suspended`, `signup_invite_only`), at `create_account` (`sign_in_not_allowed`, `signup_invite_only`), or at `finish_passkey_sign_in` (`passkey_not_accepted`, `passkey_expired`, `account_suspended`, `sign_in_not_allowed`) | `method`, `provider`?, `failure` |
| `account_created` | `create_account` or `finish_passkey_sign_up` creates an account | `account`, `method`, `provider`?, `mailbox_held` |
| `passkey_counter_regressed` | a passkey sign-in reports a signature counter that did not move forward (a cloned authenticator, or a synced one) | `passkey` |

A refusal changes nothing, so `sign_in_failed` is written in a transaction
of its own. Requests refused before a flow is looked at (CSRF, rate limits,
an unknown provider) track nothing: they are not sign-in attempts, and
tracking them would let anyone flood the outbox. Every tracked refusal sits
behind the per-address limits of its endpoint. No failure names an account:
the typed address is never kept, and a refusal says nothing about accounts.

### Sessions (instance)

| Event | Fires when | Properties |
| --- | --- | --- |
| `session_ended` | `end_session` / `end_session_post` ends a live session (`anonymous`); a session already ended tracks nothing | `session` |
| `sessions_ended` | `sign_out_everywhere` ends every session and refresh token of the account (`user`) | `account_id`, `sessions_ended`, `refresh_families_revoked` |

### Ways in and passkeys (instance)

| Event | Fires when | Properties |
| --- | --- | --- |
| `way_in_added` | a signed-in round trip (link or connect) links a provider identity that was no way in (`anonymous`) | `way_in` |
| `way_in_removed` | `remove_way_in` removes one (`user`, cause `member`); a failed mailbox hand-over takes back the way in its round trip added (`anonymous`, `connect_failed`); `prune` removes one whose mailbox never got a connection (`system`, `mailbox_never_connected`) | `way_in_id`, `account_id`, `provider`, `cause`, `sessions_ended` |
| `connected_way_in_removed` | `forget_connected_way_in` removes a way in that came only from connecting a mailbox, with its last connection (workspace-scoped: the connection's workspace; `system`) | `way_in_id`, `account_id`, `provider`, `sessions_ended` |
| `passkey_registered` | `register_passkey` adds one (`user`) | `passkey` |
| `passkey_renamed` | `rename_passkey` changes its name (`user`; the same name again tracks nothing; the name is never kept) | `passkey` |
| `passkey_removed` | `remove_passkey` removes it (`user`; the snapshot as it was) | `passkey`, `sessions_ended` |

A removed way in is gone with its row, so its removal carries ids, its
provider and counts rather than a snapshot.

### Accounts (instance, `admin`)

| Event | Fires when | Properties |
| --- | --- | --- |
| `account_suspended` | `admin_suspend_account` suspends an account (a retry tracks nothing) | `account` |
| `account_unsuspended` | `admin_unsuspend_account` lifts it | `account` |

### Apps and consents

| Event | Scope | Fires when | Properties |
| --- | --- | --- | --- |
| `client_registered` | workspace | `register_client` registers a workspace's app (`user`) | `client` |
| `client_secret_rotated` | workspace | `rotate_client_secret` gives it a new secret (`user`) | `client` |
| `client_removed` | workspace | `remove_client` removes it (`user`, cause `removed`), or `revoke_departed_consents` removes an app the departed member registered (`system`, `member_left`) | `client_id`, `cause`, `consents_revoked` |
| `dynamic_client_registered` | instance | `/oauth2/register` registers a client (`anonymous`) | `client` |
| `consent_granted` | instance | the consent screen's Allow (`anonymous`) | `consent` |
| `consent_denied` | instance | the consent screen's Deny (`anonymous`) | `account_id`, `client_kind`, `client_id`? |
| `consent_revoked` | instance | `revoke_consent` takes a consent back (`user`) | `consent` |
| `consents_revoked` | workspace | `revoke_departed_consents` revokes the consents a departed member gave in the workspace (`system`); none tracks nothing | `member_id`, `consents` |
| `held_grant_discarded` | instance | `discard_held_grant` (`user`) | `held_grant_id`, `account_id`, `provider` |
| `held_grant_adopted` | instance | `adopt_held_grant` hands it to a workspace (`user`) | `held_grant_id`, `account_id`, `provider`, `in_workspace_id` |

### Tokens (instance, `anonymous`)

| Event | Fires when | Properties |
| --- | --- | --- |
| `tokens_issued` | `/oauth2/token` exchanges an authorization code (a client got tokens for a sign-in) | `account_id`, `session_id`?, `client_kind`, `client_id`?, `refresh_token_issued`, `scope_count`, `workspace_bound` |
| `refresh_reuse_detected` | a spent code or rotated refresh token is presented again, and its family is revoked | `account_id`, `client_kind`, `client_id`? |
| `refresh_token_revoked` | `/oauth2/revoke` revokes a live refresh family (an unknown or already revoked token tracks nothing) | `family_id`, `account_id`, `client_kind`, `client_id`? |

**Token issuance, decided.** `tokens_issued` fires for the
`authorization_code` grant only: once per sign-in per client, the moment an
app gets tokens. Two grants are deliberately not tracked:

- `refresh_token`: each session rotates its refresh token every few minutes
  (the access token lives ten); one event per rotation would make the
  outbox mostly noise and say nothing new. What a rotation stands for is
  tracked when it starts (`tokens_issued`), when it is abused
  (`refresh_reuse_detected`) and when it ends (`refresh_token_revoked`,
  `session_ended`, `sessions_ended`, `consent_revoked`, `account_suspended`).
- `client_credentials`: a workspace app asks for a new ten-minute token as
  often as it likes, with no transaction to track in; the app's lifecycle
  (`client_registered`, `client_secret_rotated`, `client_removed`) is
  tracked instead.

A refused exchange (`invalid_grant`, `invalid_client`, …) tracks nothing,
except reuse, which revokes tokens.

## Coverage

Operations (`openapi/openapi.json`, tag `identity`; `admin.json` for the
`admin_*`):

| Operation | Events / exemption |
| --- | --- |
| `finish_sign_in` (GET, changes state) | `sign_in_succeeded`, `sign_in_failed`, `way_in_added`, `way_in_removed` |
| `create_account` | `account_created`, `sign_in_succeeded`, `sign_in_failed` |
| `finish_passkey_sign_in` | `sign_in_succeeded`, `sign_in_failed`, `passkey_counter_regressed` |
| `finish_passkey_sign_up` | `account_created`, `sign_in_succeeded` |
| `end_session` (GET), `end_session_post` | `session_ended` |
| `sign_out_everywhere` | `sessions_ended` |
| `remove_way_in` | `way_in_removed` |
| `register_passkey` / `rename_passkey` / `remove_passkey` | `passkey_registered` / `passkey_renamed` / `passkey_removed` |
| `register_client` / `rotate_client_secret` / `remove_client` | `client_registered` / `client_secret_rotated` / `client_removed` |
| `register` | `dynamic_client_registered` |
| `answer_consent` | `consent_granted`, `consent_denied` |
| `revoke_consent` | `consent_revoked` |
| `discard_held_grant` / `adopt_held_grant` | `held_grant_discarded` / `held_grant_adopted` |
| `token` | `tokens_issued`, `refresh_reuse_detected` |
| `revoke` | `refresh_token_revoked` |
| `admin_suspend_account` / `admin_unsuspend_account` | `account_suspended` / `account_unsuspended` |
| `start_sign_in` (GET) | exempt: only starts a round trip; `finish_sign_in` tracks its outcome |
| `start_link` | exempt: only issues a ticket for a round trip; `finish_sign_in` tracks its outcome |
| `start_passkey_registration` | exempt: only starts a ceremony; `register_passkey` tracks its outcome |
| `start_passkey_sign_in` / `start_passkey_sign_up` | exempt: only start a ceremony; `finish_passkey_sign_in` / `finish_passkey_sign_up` track its outcome |
| `authorize` (GET) | exempt: issues a short-lived code for a session that exists, or sends the browser to sign in; the code's redemption is tracked by `token` |

Event handlers:

| Handler | Events / exemption |
| --- | --- |
| `forget_connected_way_in` | `connected_way_in_removed` |
| `revoke_departed_consents` | `consents_revoked`, `client_removed` |
| `name_connected_mailbox` | exempt: bookkeeping, a way in's mailbox record named with the connection the connectors module confirms (tracked there); nothing anyone did changes |
| `notify_way_in_added` | exempt: sends the notice of `identity.way_in_added`, which is tracked where the way in is added |
| `remove_workspace_clients` | exempt: consequence of `workspaces.workspace_deleting` (tracked by `workspaces`): the deleted workspace's apps and their access go with it |

Schedules: `prune` → `way_in_removed` (a way in whose mailbox never got a
connection; everything else it deletes is expired sign-in state, ended
sessions and revoked tokens kept a day, maintenance with no action of
anyone).

## Notes

- `sign_in_succeeded` and `account_created` are tracked in the transaction
  that starts the session or creates the account, next to the audit row and
  the domain event they already write.
- `session_ended`, `refresh_token_revoked` and `passkey_renamed` need to
  know whether anything changed: `sessions::end_for`,
  `tokens::revoke_token` and the rename now say so.
- Patch numbers and plan versions are taken in merge order; this one builds
  on plan version 2 (`0002-conversations`).
