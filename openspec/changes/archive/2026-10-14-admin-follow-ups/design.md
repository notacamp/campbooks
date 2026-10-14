# Design

## Context

Three small follow-ups to the admin dashboard (ADR 0015) and one deferred
item. Each touches a different owner: the People list belongs to the
`workspaces` module, suspension to `identity`, the job queue to the kernel,
and the Codes copy to the client.

## Data ownership, permissions and events

- **Suspension** stays `identity.accounts.disabled_at`, read only by the
  identity module. The new `identity.admin_list_accounts` reads it inside
  `begin_admin` like `identity.admin_get_account`, needs `users.read` (or
  `super`), and answers ids and suspension times only. It is audited by the
  admin layer like every admin request. No new table, policy or event.
- **People** stays the workspaces module's `workspaces.users`; its list is
  unchanged. The client puts the two answers side by side by
  `account_id` (= the token `sub` = `identity.accounts.id`), as the person
  page already does.
- **Job errors**: `kernel.jobs.last_error` keeps its column and meaning; only
  what the worker writes changes. No migration.
- No new scopes, permissions, events or settings.

## Decisions

### The People list asks identity per page

The workspaces module's list cannot read `identity.accounts` (schema per
owner, no cross-schema joins), so the list asks the identity module which of
the accounts on screen are suspended. The identity module exports a
render-prop part (`SuspendedAccounts`) that the app's module manifest hands
to the workspaces module's People route, exactly as it hands over the
person page's `AccountAdminPanel`; modules still import nothing from each
other. The lookup runs one request per chunk of 50 ids (the list's page
size), so pages already shown keep their cached answer as more load, and a
suspend or unsuspend invalidates them. While the answer is unknown, or if it
fails, the list shows without badges: the person's page remains the
authoritative view.

The badge is the existing `lib/ui` `Tag` with `tone="danger"` in the
`ListRow` badge slot, worded like the person page ("Suspended",
"Suspensa", "Suspendida", "Suspendu").

### `last_error` is sanitized where it is written

The worker's failure path is the only writer of `last_error` with a
handler's message (the lease-expiry sweep writes a fixed sentence), and the
same string is logged. Sanitizing there covers every handler, present and
future, without touching any of them:

1. keep the first line only (provider replies and bodies span lines), with
   ` […]` when more followed;
2. replace anything quoted (`"…"`, `'…'`, `` `…` ``, `“…”`, `‘…’`, `«…»`) by
   `"…"`; an apostrophe inside or after a word is not a quote; a quote that
   never closes hides the rest of the line;
3. replace email addresses by `[address]`;
4. cut URLs to scheme and host (no user info, path or query);
5. replace opaque runs of 40+ letters, digits and `+/=_-.` (tokens, encoded
   bodies; longer than a UUID so ids stay) by `[…]`;
6. turn control characters into spaces, then bound to 500 characters.

What an administrator needs stays: the handler's own sentence, ids, status
codes, SQLSTATEs. The `JobError` contract does not change (still "no content
in the message"); this is the backstop for when a handler slips.

### The Codes copy describes uses, not "single use"

Codes have a number of uses (default 1). The lead now says so and how a
code stops (used up, expired, revoked), in four locales with full
diacritics. "Single-use" remains only where it is literally true (the
recording that mints a 1-use code, a migration comment that cannot change).

## Alternatives considered

### Suspended badge

- **A. A suspension projection in the workspaces module** (a
  `workspaces.users.suspended_at` column fed by `identity.account_suspended`
  and a new `account_unsuspended` event). The list would sort and filter by
  it server-side, but it adds a migration, a new event, a consumer, a second
  copy of identity's data that can lag, and edits identity's event set,
  which the open identity PR is also changing. Rejected: two sources of
  truth for a badge.
- **B. One `identity.admin_get_account` call per row.** No new endpoint, but
  50 requests (and 50 admin-audit rows) per page. Rejected.
- **C. A batch lookup in identity (chosen).** One request per page, identity
  stays the only reader of its table, nothing to keep in sync.
- **D. A hook passed as a part (`useSuspendedAccounts`)** instead of a
  render-prop component. Shorter, but a hook passed through props is a
  dynamic hook to the React compiler lint; the render-prop part keeps the
  contract a component like `AccountPanel`.

### Job errors

- **A. Typed errors** (`JobError` built only from `&'static str` codes and
  typed ids/numbers, so `Display` is content-free by construction). The
  strongest guarantee, but it changes ~90 construction sites across every
  module, several of which the open security-audit PRs are editing, and it
  still needs an escape hatch for third-party error types. Recorded as the
  long-term option; not done here.
- **B. Sanitize and bound at write time (chosen).** One function at the one
  writer, covers handlers that do not exist yet, keeps messages useful.
  Heuristic: an unquoted name (`Maya` in `Maya <[address]>`) can survive, so
  the "no content" contract stays and handlers keep using `redact::*` for
  the errors they wrap.
- **C. Store only the error class** (`retry`/`fatal`, or the handler's
  name). Content-free by construction but useless to an administrator
  deciding whether a retry will help. Rejected.
- **D. Hide `last_error` from the admin API** and keep it for logs. Moves
  the problem to the logs. Rejected.

### Codes copy

Calling the code "single-use" and making the default 1 the only option was
considered and rejected: S1 deliberately supports team codes with several
uses (the demo data has one), so the copy follows the feature.

### Copied from the Rails app on `main`?

No. The Rails app had a users table with a status column in the same
database (`/admin/users`) and showed raw exception messages in Mission
Control. Neither carries over: suspension stays identity's, and job errors
are sanitized.

## Deferred: suspension on access tokens already issued (#122 item 1)

Not built in this change, at the coordinator's request: the open
security-audit PRs rework the kernel's extractors (a deny-by-default
`authorize(principal, action, resource)`) and identity's sessions and
refresh families ("sign out everywhere", which leaves the same ten-minute
access-token gap). The options to weigh when it is built on top of them:

- **Per-request account check through a kernel port** implemented by
  identity and wired by the server (like `AdminProfiles`), at the single
  choke point `TokenVerifier::verify` (or `authorize`). One primary-key read
  per request, the same order as the membership read every workspace
  request already makes. An in-process cache needs care: events are
  delivered to one consumer, not to every API process, so an
  event-invalidated cache is only correct with one process; a short TTL or
  `LISTEN/NOTIFY` is needed otherwise.
- **A per-account revocation epoch** (`tokens_not_before`) compared with the
  token's `iat`: same read cost, also serves "sign out everywhere", and
  unsuspending does not revive tokens issued before the suspension (a plain
  `disabled_at` check would).
- **Shorter access tokens** (e.g. two minutes): no new machinery, but a
  window remains and refresh traffic grows five-fold.

Until then: a suspended account's web sessions end at once (BFF), its
refresh tokens are revoked and it cannot sign in, but a native client's
access token issued before the suspension works until it expires, at most
ten minutes later.

## Risks

- A message whose only useful part was quoted (`unknown label "inbox"`)
  loses that part. Handlers that want an identifier shown should put it
  unquoted (ids, codes), which is the existing convention.
- The badge depends on a second request; if identity's lookup fails the
  list shows no badge rather than an error.
