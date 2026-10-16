# Design

## Context

The API is an OAuth resource server (ADR 0004): it checks a JWT's signature,
issuer, audience and expiry and asks nobody else. The in-process issuer
(ADR 0014) gives a person's access token ten minutes. Suspension
(`identity::domain::suspension`) and sign out everywhere
(`identity.sign_out_everywhere`) end issuer sessions and refresh-token
families, and the BFF drops the account's web sessions on their events, but
an access token already in a client's hands keeps verifying until `exp`.

What was checked on `next` before deciding:

- **One choke point.** Every bearer token goes through
  `TokenVerifier::verify`. Its callers: `Scoped<S>` (and through it
  `Unbound<S>` and `Workspace<S>`), `Admin<P>`, the idempotency layer (which
  verifies once and attaches the principal for `Scoped` to reuse) and the
  request locale. `kernel::authz::authorize` runs for single-record actions
  only, so it is not a per-request hook.
- **The web BFF** forwards each call through the in-process API router with
  the session's stored access token in `Authorization`, so its calls meet
  the same `Scoped`/`Admin` extractors and the same verifier. Nothing to add
  there; a test covers it.
- **Several processes.** Events reach one consumer (job queue), and the API
  and the worker may run as separate processes. Anything kept in memory and
  cleared only by an event handler would be stale in every other process.
- **Data.** `identity.accounts` carries no row-level security (it is not
  workspace-owned); the runtime role reads it with a plain query, as
  `accounts::is_suspended` does.
- **The change stream** resolves membership again at least once a
  heartbeat (25 s by default), busy or idle, and ends with `revoked` when it
  is gone. It ends at the token's `exp` at the latest.

## Data ownership, events, scopes

- `identity.accounts.tokens_not_before timestamptz NULL` — owned by
  `identity`, read by identity only (the port's implementation runs in the
  identity crate). `NULL`: nothing revoked yet.
- Written by `suspension::suspend` and `sign_out_everywhere`, in their
  existing transactions, through `accounts::revoke_tokens`; read by the
  port and by the token endpoint. Unsuspend does not touch it.
- No new event, scope, permission, endpoint or setting. The existing
  `identity.account_suspended` and `identity.signed_out_everywhere` events
  are unchanged; nothing needs them for this.

## Decisions

### A revocation epoch, compared with `iat`

The account carries the instant of its last revocation,
`tokens_not_before`. Two rules hang off it:

- **The API** accepts a person's token only when its `iat` falls in a
  **later second** than the epoch (`iat > floor(tokens_not_before)`). `iat`
  has one-second resolution, so a token minted earlier in the revocation's
  own second carries the same second and is refused, like every token minted
  before it.
- **The token endpoint** refuses (`invalid_grant`) to mint anything for a
  sign-in that is not newer than the epoch (`auth_time <= tokens_not_before`),
  whatever code or refresh token it holds: a revoked sign-in is dead even if
  a code of it was still unredeemed. For a sign-in that *is* newer, a token
  minted in the revocation's own second would carry that second and be
  refused, so the issuer stamps it `iat = floor(tokens_not_before) + 1`
  instead (at most one second ahead of the clock, well inside the verifier's
  leeway; `nbf` stays the real time). A person who signs in again right
  after a revocation gets a token that works; a token from an older sign-in
  never gets the later stamp.

Races with a revocation that is committing:

- Suspension already locks the account row (`FOR UPDATE`); sign out
  everywhere now does too. Both go through `accounts::revoke_tokens`, which
  takes the epoch **after** the lock, from the later of the caller's `now`
  and the wall clock, and only ever moves it forward (`GREATEST`).
- The token endpoint reads the account (the profile it mints from, now with
  the epoch) inside its transaction **under `FOR SHARE`**. Either the
  revocation holds the row first: the exchange waits, then finds the account
  suspended or its sign-in older than the epoch, and mints nothing. Or the
  exchange holds it first: the revocation waits for it to commit and then
  takes an epoch later than the time that exchange started, which is what
  its token's `iat` carries, so the API refuses that token.
- What remains is clock agreement between processes: the epoch, `iat` and
  `auth_time` all come from the application clock, as `exp` already does.
  NTP keeps processes well inside the margins involved.

Unsuspending does not clear the epoch, so tokens from before the suspension
never come back to life; the person signs in again and that newer sign-in
gets tokens.

### A kernel port checked inside the verifier

```text
pub trait TokenRevocation: Debug + Send + Sync + 'static {
    fn standing<'a>(&'a self, db: &'a PgPool, account: &'a str) -> StandingFuture<'a>;
}
pub struct TokenStanding { pub suspended: bool, pub tokens_not_before: Option<DateTime<Utc>> }
```

`TokenVerifier::with_revocation(db, port)` attaches it; the server does so
only for the in-process issuer's verifier (`with_composed_ports`), next to
the client directory, mirroring how `AdminProfiles` and `ClientDirectory`
are defined in the kernel, implemented by a module and wired by the server.
No module imports identity.

`verify` then, for `PrincipalKind::User` tokens only: reads the standing
(one primary-key `SELECT` on `identity.accounts` with the runtime role) and
refuses with `AuthFailure::InvalidToken` when the account is missing,
suspended, the token has no `iat`, or `iat` is before the epoch. A database
error fails closed (the request fails; it is never let through). With no
port (an outside issuer, tests without identity) nothing changes.

`Principal` gains `issued_at` (from `iat`), which the verifier already
checked against the clock but dropped.

**Same answer as any bad token.** A revoked token gets exactly what a
forged or foreign one gets: 401, `code` `invalid_token`,
`WWW-Authenticate: Bearer error="invalid_token"`. Nothing says the account
is suspended. (`Admin<P>` answers 404 for every token it cannot verify;
that does not change, so a revoked token there gets 404 like any other
bad token.) The reason is logged at debug level only, as for every
refusal, with no token or claim.

### No cache

The read is one indexed primary-key lookup per authenticated request, of
the same order as `Workspace<S>`'s uncached membership transaction that
most of those requests already run. A cache would need either a TTL (a
window during which revoked tokens still work, in every process) or a
cross-process invalidation channel (new LISTEN/NOTIFY machinery). Nothing
measured asks for either, so there is none; revocation takes effect on the
next request in every process.

### Change streams end within a heartbeat

The stream's membership check (`check_access`, run at least once a
heartbeat and before any page that touches membership) first asks
`TokenVerifier::still_stands(&principal)`, the same rule through the same
port. When the token no longer stands the stream sends `revoked` and ends,
exactly as for a removed member; reconnecting with the token answers 401.
The bound is one heartbeat interval (25 s by default), much shorter than
the ten-minute token lifetime. Waking every open stream at the moment of
revocation would need a cross-process signal from identity to the change
hub; one heartbeat was judged enough.

## Alternatives considered

1. **Check `disabled_at` only (an "account standing" port).** Same cost,
   no migration. But unsuspending within ten minutes would bring the
   suspended person's old tokens back to life, and it does nothing for sign
   out everywhere, which has the same gap. Rejected.
2. **Cache standings in memory, invalidated by the events.** Saves the
   read, but events reach one consumer in one process: every other API
   process keeps the stale entry until a TTL, so the gap shrinks without
   closing. Closing it needs a cross-process broadcast (LISTEN/NOTIFY), new
   machinery for a cost nobody measured. Rejected for now; the port leaves
   room to add a ≤ 5 s cache later if a profile ever shows the read.
3. **A per-account generation number in a custom claim** (`gen`, bumped by
   each revocation, compared exactly). No clock rounding at all, but a
   custom claim every token must carry, a lookup either way, and nothing
   for tokens minted before the claim existed. The epoch uses the standard
   `iat`; the one-second rounding is handled at minting. Rejected.
4. **Token introspection or a deny-list of `jti`s.** Introspection makes
   every request a round trip to the issuer (in-process it is the same read
   with more machinery); a `jti` list must record every live token at mint
   time to revoke them all. Rejected.
5. **Shorter access tokens (e.g. two minutes).** No new code, but still a
   window, more refresh traffic, and more load on the issuer. Rejected as
   the fix; it could still complement this one.

Self-host weight: one nullable column, no new process, setting or
dependency.

**Rails app on `main`:** not a copy. The old app used cookie sessions
looked up on every request and Doorkeeper tokens checked against its
database; there was no stateless token to revoke. The epoch-versus-`iat`
check is chosen on its merits for JWTs.

## Risks

- **Clock skew between processes** greater than a second could let a
  concurrently minted token survive (see above). The same assumption already
  holds for `exp`.
- **Database unavailable** now fails requests with a person's token that
  would have passed on the signature alone. Those requests mostly need the
  database anyway.
