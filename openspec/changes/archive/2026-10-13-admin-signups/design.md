# Design

## Data ownership and events

`workspaces` owns both tables (schema `workspaces`); nothing else reads or
writes them.

- `signup_requests (user_id uuid primary key → workspaces.users, status
  'pending'|'approved'|'rejected', requested_at, decided_at, decided_by text,
  reason text ≤ 500)`. `decided_by` is the administrator's identity account
  id (by id only). Row-level security, forced: a user sees and files only their
  own request (`campbooks.user_id`); an administrator reads and decides every
  request through `begin_admin` (policies on `campbooks.admin_reads`). The
  table holds ids, status and times only, so it joins the pinned admin-read
  allowlist; names and addresses come from `workspaces.users` (same schema, no
  RLS, as today).
- `signup_codes (id uuid, digest bytea unique, label ≤ 100, max_uses 1..100000,
  uses ≤ max_uses, expires_at, created_by text, created_at, revoked_at)`. No
  RLS: it is instance policy, holds no personal data (a digest of a
  80-bit random code), and is touched only by this module — like
  `workspaces.users`. No admin-read policy is therefore needed or added.
- Security log (`workspaces.security_events`): `signup_approved` /
  `signup_rejected` (instance-level, the requester's user id, `decided_by`
  in metadata), `signup_code_redeemed` (in the new workspace's log, with the
  code's id). The decision is recorded in the requester's own instance-level
  log by setting `campbooks.user_id` in the admin transaction after the
  update; no policy is widened for it.
- No domain events: nothing else reacts to a signup decision yet.

## Policy flow

`signup::admit` replaces `check_signup`:

- `open`: allowed. Members and the instance's first user are always allowed.
- `invite_only`: refused (`signup_invite_only`).
- `approval`: request `approved` → allowed; `rejected` → `signup_rejected`;
  `pending` → `signup_requires_approval`; none → `INSERT … ON CONFLICT DO
  NOTHING` plus a `signup_requested` log entry only when the row was new, then
  `signup_requires_approval`. Retrying creates no second request or entry.
- `code`: an environment code (constant-time digest comparison, reusable,
  unchanged) → allowed; otherwise the digest of the typed code (whitespace
  and dashes ignored, case-insensitive) is spent **inside the creation
  transaction**: `UPDATE signup_codes SET uses = uses + 1 WHERE digest = $1
  AND revoked_at IS NULL AND uses < max_uses AND (expires_at IS NULL OR
  expires_at > now()) RETURNING id`. Concurrent attempts at the last use
  serialize on the row lock and the loser's re-check finds `uses = max_uses`
  (tested with six simultaneous requests). If creation fails later, the
  transaction rolls back and the use is given back. Zero rows is
  `signup_code_invalid` whatever the reason (unknown, spent, revoked,
  expired), so a probe learns nothing.

Minted codes look like `K7QM-2XWP-9DHE-RT4A`: 16 characters of a 32-letter
alphabet without `0 1 I O` (80 bits), from the system RNG. Only the SHA-256
of the normalised code is stored; the plaintext is in the mint response only.

## Admin API (module `workspaces`, permission `signups.review`)

| operationId | route |
| --- | --- |
| `workspaces.admin_list_signup_requests` | `GET /admin/v1/workspaces/signup-requests?status&after&limit` |
| `workspaces.admin_approve_signup_request` | `POST …/signup-requests/{user_id}/approve` |
| `workspaces.admin_reject_signup_request` | `POST …/signup-requests/{user_id}/reject` `{reason?}` |
| `workspaces.admin_list_signup_codes` | `GET …/signup-codes?after&limit` |
| `workspaces.admin_mint_signup_code` | `POST …/signup-codes` `{label, max_uses?, expires_at?}` |
| `workspaces.admin_revoke_signup_code` | `POST …/signup-codes/{code_id}/revoke` |

- Lists page by keyset cursor (`after` is the opaque sort key of the last row,
  `limit` 1..200, default 50). Requests order: pending first, then newest
  first; codes: newest first.
- Approve, reject and revoke are idempotent: repeating one returns the same
  state (and writes no second log entry). Approving a rejected request, or the
  reverse, is allowed: administrators may change their mind. Unknown ids are
  404.
- Responses are metadata: names and addresses the person signed in with,
  statuses, times, counts. Never the code, its digest, or anything from a
  workspace.

## Member API

`GET /api/v1/signup` (scope `workspaces:read`): `{ policy, restricted,
request }`. `restricted` is false for members, the first user and `open`
instances; `request` is the caller's own status under `approval`. The create
screen reads it to say "waiting", "approved — continue" or "not approved"
before and after an attempt. Creating a workspace stays the only way to file
a request (as before), so no new write path exists for members.

## Client

`modules/workspaces` contributes the Signups admin panel (`SIGNUPS_ADMIN_SECTION`,
one line in `ADMIN_SECTIONS`): a Requests / Codes switch; requests filtered by
status with Approve / Reject behind an inline confirmation (Reject takes an
optional reason); codes: a mint form, the minted code shown once in a
`CopyField` with an explanation, and a list with uses, expiry, state and
Revoke behind a confirmation. The repo's confirmations are inline
(`InlineConfirm`), not modal dialogs; this follows that convention instead of
adding a dialog component. `CreateWorkspace` gains the approved and rejected
views and its "Check again" re-reads the status instead of retrying creation.

## Alternatives considered

1. **Where the requests live.** (a) Chosen: a `signup_requests` table in
   `workspaces`, beside the policy that files it; one row per user, status
   updated in place, the history in the security log. (b) Derive "requests"
   from `security_events` of kind `signup_requested` and store decisions as
   more events: no new table, but every read would fold an event stream, the
   "pending" set would be a query over unrelated events and the RLS for
   events (workspace-scoped) does not fit. (c) A request per attempt:
   duplicates for impatient users, which the brief rules out. (a) wins:
   smallest moving parts that answer "who is waiting" with an index.
2. **Single-use enforcement for codes.** (a) Chosen: one conditional
   `UPDATE … RETURNING` inside the creation transaction. Atomic by row lock,
   no new machinery, and a failed creation gives the use back. (b) Check
   first, mark used afterwards (or in a job): racy; two people can both
   create a workspace with the last use. (c) An advisory lock or
   `SERIALIZABLE` around creation: heavier and still needs the counter. (a)
   wins.
3. **Where codes are stored and how.** (a) Chosen: digest only, plaintext
   returned once. (b) Keep reversible ciphertext to let administrators re-read
   a code: a secret the database would hold for no operational need. (c)
   Keep the environment list as the only source: not revocable, not
   single-use, not auditable. Environment codes stay as the legacy path for
   self-hosters who rely on them.
4. **Admin writes under row-level security.** (a) Chosen: an `admin_reads`
   policy for SELECT and UPDATE on `signup_requests` only, used through
   `begin_admin`, and codes (no RLS) accessed by this module's own queries
   behind `Admin<SignupsReview>`. (b) A `SECURITY DEFINER` function for each
   decision: narrower on paper, but needs a privileged owner and moves SQL
   out of the slice. (c) Widen `security_events`' policies for the decision
   log: it would put the security log in the admin-read allowlist; instead
   the transaction acts as the requester for that one insert.

Copied from the old Rails app? Only the idea of "a pending list an admin
approves" and "single-use beta codes" (`/admin/signup_requests`,
`/admin/beta_codes`). The data model, the atomic redemption, the digest-only
storage, the policy-by-attempt filing, the permission model and the UI are
new, chosen on the merits above.

## Risks and trade-offs

- A single environment code still lets anyone who learns it in, forever; that
  is the legacy path and is documented as such. Administrators can move to
  minted codes without changing the policy.
- An administrator who approves from stale data may approve someone who has
  since joined by invitation: harmless (the request is simply unused).
- `signup_requests` rows are not purged; one small row per person who ever
  asked. They go with the user.
