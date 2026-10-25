# Design

How to add a tracked event is in `docs/tracking.md`; the decision is
ADR 0020. This change applies it to the web BFF (`crates/server/src/bff/`).

## Data ownership and events

The BFF owns the schema `bff`: `bff.sessions`, `bff.retired_tokens` and
`bff.login_transactions`, reached only through the `bff.*` functions
(campbooks-cloud#212). Sessions are instance-wide, so every event is
instance-wide. Events go to `kernel.tracked_events`; nothing else is
written. The handlers consume `identity.signed_out_everywhere` and
`identity.account_suspended` as before; no domain event is published, no
endpoint, scope or AI role changes.

**Where the events are generated.** Event names are
`<module>.<object>_<action>` and the module must own the code. The BFF is
no crate under `crates/modules`; its operations, jobs and owner are all
`bff.*`. The plan tool gains `bff` as a third platform module next to
`kernel` and `server`: its types are generated into the kernel's
`tracking.gen.rs` (as the server's are) and the check looks for their use
in `crates/server/`. The alternative, events named `server.web_session_*`,
needs no tool change but names no module anyone thinks in, and differs from
the operations (`bff.logout`) that cause them.

**Transactions.** Starting and ending a session ran as single statements
on the pool. They now run in a transaction opened by `bff::store` (the one
BFF file allowed to, `architecture.toml`; `begin_refresh` becomes `begin`):
the `bff.*` call, then `Tracker::track`, then the commit. The refused
refresh already deleted its session inside the refresh's transaction and
tracks there. The handlers end the sessions in the job's own transaction
(`ctx.tx()`) and track with `ctx.track`. `bff.end_session` and
`bff.end_account_sessions` already say whether, and how many, sessions
ended, so a session already gone tracks nothing.

What an event holds: the session id (`bff.sessions.id`), the account id
and the cause. The account id is the session's subject (from the session
row the request looked up, which never changes) when it is a Campbooks
account id, a UUID (ADR 0014); an external identity provider's subject
that is not one is left out. Never a name, an address (both are stored on
the session), the IdP session id, a token or token hash, the user agent,
the return path or an IP address.

## Events

All instance-wide.

| Event | Fires when | Actor | Properties |
| --- | --- | --- | --- |
| `bff.web_session_started` | `bff.callback` finishes a sign-in and stores the new session | `anonymous` | `session_id`, `account_id`? |
| `bff.web_session_ended` | one web session ends (cause below); one already gone tracks nothing | `anonymous` | `session_id`, `account_id`?, `cause` |
| `bff.web_sessions_ended` | the sessions of an account, or of one IdP session, end at once (cause below); none ended tracks nothing | `anonymous`, `system` | `account_id`?, `cause`, `sessions_ended` |

`bff.web_session_end_cause`:

| Value | Where |
| --- | --- |
| `logout` | `bff.logout`: the person signed out |
| `replaced` | `bff.callback`: the browser signed in again; its previous session ends |
| `copied_cookie` | a lookup (`bff.session`, a proxy, `bff.logout`'s): a rotated-away token came back after its successor was used, or a retired one came back at all; the session ends (its account is unknown for a retired token: only the session id is found) |
| `scopes_outdated` | a lookup: the session signed in before the BFF asked for scopes it asks for now |
| `refresh_refused` | a proxy: the identity provider refused to refresh its tokens |
| `tokens_unreadable` | a proxy: its stored tokens no longer decrypt (`CAMPBOOKS_SECRET_KEY` changed) |
| `access_expired` | a proxy: its access token expired and it holds no refresh token |

`bff.web_sessions_end_cause`:

| Value | Where |
| --- | --- |
| `backchannel` | `bff.backchannel_logout`: the identity provider's logout token named them (by IdP session, or every session of the account; with only an IdP session named, no `account_id`) |
| `signed_out_everywhere` | `bff.end_signed_out_sessions` ← `identity.signed_out_everywhere` |
| `account_suspended` | `bff.end_suspended_sessions` ← `identity.account_suspended` |

The BFF's own routes authenticate with the session cookie, not a token, so
the HTTP layer leaves them `anonymous` (as the issuer's sign-in and logout
routes are, campbooks-cloud#326); the account is in the body. The handlers
run as `system` (their kind), on behalf of whoever caused the identity
event (the person signing out everywhere, the administrator suspending),
with that request's correlation.

## The proxies

`/bff/api/*` and `/bff/admin/*` run the request through the API's (or the
admin API's) own router in-process. That router has the HTTP tracking layer
of `operations::compose` (and of `admin_routes`): it opens a context of its
own for the matched operation, taking the enclosing correlation, and
`Scoped` (or `Admin`) names the caller from the session's access token. So
an event the forwarded operation tracks carries that operation's id
(`identity.admin_suspend_account`, not `bff.admin_post`) and the person
(`user`, account id) or the administrator (`admin`, account id). Nothing
needed fixing; tests pin it.

## Coverage

Operations (all in `openapi/openapi.json`, tag `bff`):

| Operation | Events / exemption |
| --- | --- |
| `bff.callback` (GET, changes state) | `web_session_started`, `web_session_ended` |
| `bff.logout` | `web_session_ended` |
| `bff.backchannel_logout` | `web_sessions_ended` |
| `bff.session` (GET, may end a session) | `web_session_ended` |
| `bff.api_get`, `bff.api_post`, `bff.api_put`, `bff.api_patch`, `bff.api_delete` | `web_session_ended` |
| `bff.admin_get`, `bff.admin_post`, `bff.admin_put`, `bff.admin_patch`, `bff.admin_delete` | `web_session_ended` |
| `bff.login` (GET) | exempt: only starts a sign-in round trip (a short-lived sign-in transaction); `bff.callback` tracks its outcome |

The proxies list `web_session_ended`, not an exemption: the session they
look up may end on the way (a copied cookie, outdated scopes, a refused
refresh, unreadable tokens, an expired access token), and a row that says
"tracks none" would be false. What they forward is tracked by the
operation reached, under its own id (above).

Event handlers:

| Handler | Events / exemption |
| --- | --- |
| `bff.end_signed_out_sessions` | `web_sessions_ended` |
| `bff.end_suspended_sessions` | `web_sessions_ended` |

Schedules: `bff.prune_sessions` is exempt: maintenance, it deletes
sessions already past their idle or absolute end and sign-ins never
finished; no action of anyone.

Nothing else in the server crate registers a job, schedule or handler (the
kernel's own are the kernel's).

## Notes

- `store::end_session` (on the pool) gives way to `delete_session` in a
  transaction; `insert_session` and `delete_for_logout` take a connection.
- Patch numbers and plan versions are taken in merge order; this one builds
  on plan version 3 (`0003-identity`).
