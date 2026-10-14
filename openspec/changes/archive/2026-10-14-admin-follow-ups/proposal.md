# Proposal

## Why

Follow-ups from the admin dashboard (campbooks-cloud#95, slices S1–S3),
tracked in campbooks-cloud#122:

- Admin → People lists people without saying which accounts are suspended;
  only a person's own page says so. An administrator scanning the list cannot
  tell who is locked out.
- `kernel.jobs.last_error` is shown verbatim in Admin → Jobs and logged. The
  rule "never put content in a `JobError`" is a convention only: a handler
  that quotes a provider's reply, an address or a link into its message puts
  that text in front of every administrator and in the logs.
- Admin → Signups → Codes calls the codes "single-use" and, in the same
  sentence, says each "can be used as many times as you set". Codes carry a
  number of uses (1 by default); the copy contradicts itself in all four
  locales.

## What Changes

- **Suspended badge on People.** The identity module serves
  `GET /admin/v1/identity/accounts?ids=…` (`identity.admin_list_accounts`,
  `users.read`): for up to 200 account ids, each account's id and suspension
  time, nothing else. The People list asks once per page of people and marks
  suspended accounts with a "Suspended" tag (four locales). The identity
  module contributes the lookup to the workspaces module's list as a part
  handed in by the app's module manifest, as it already does for the person
  page; suspending or unsuspending refreshes it.
- **`last_error` sanitized where it is written.** The job worker stores and
  logs `campbooks_kernel::redact::job_error(message)`, bounded to 500
  characters (was 2,000): only the first line, anything quoted replaced by
  `"…"`, email addresses by `[address]`, URLs cut to scheme and host, opaque
  runs of 40+ token characters by `[…]`, control characters by spaces. The
  handler's own wording, ids and status codes stay. A commit failure is
  described by `redact::sqlx`.
- **Codes copy.** The Codes lead says each code works for the number of
  signups set when it is minted (1 for a single person) and stops once used
  up, expired or revoked, in en, pt-PT, es-ES and fr. Code comments and the
  unreleased CHANGELOG entry say the same.

## Deferred

- **Suspension on access tokens already issued** (item 1 of #122). A
  suspended account's access tokens still verify statelessly for up to ten
  minutes. It is deferred, not dropped: the open security-audit PRs rework
  exactly that area (a kernel `authorize(principal, action, resource)` with
  deny-by-default that changes the extractors, and identity sessions and
  refresh families with "sign out everywhere", which has the same ten-minute
  gap). Building a per-request account check now would collide with both and
  be redone on top of `authorize`. The options weighed so far are recorded in
  design.md for that change.

## Non-goals

- No change to how suspension works (sessions, refresh families, BFF).
- No filter "suspended only" on People, and no suspension column in the
  workspaces module's tables (suspension stays identity's data).
- No typed error enum for every job handler (see design.md); the contract
  "no content in a `JobError`" stays, the sanitizer is its backstop.
- Existing `last_error` rows are not rewritten; failed jobs are pruned by
  retention.

## Impact

- New admin operation `identity.admin_list_accounts` (`openapi/admin.json`,
  client `admin.gen.ts`), reason `too_many_accounts` in four locales.
- `kernel::redact::job_error`, `jobs::MAX_ERROR_LEN` = 500, worker failure
  path.
- Client: identity `SuspendedAccounts` part, workspaces People list,
  workspaces i18n (badge + Codes lead).
