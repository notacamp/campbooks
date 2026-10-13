# Design

ADR 0015 is the decision record for instance administration; this file
covers slice S2: who uses the instance, and suspending an account.

## Data ownership

Each module answers for its own tables, under its own admin router
(`/admin/v1/<module>/…`), and references the others by id only. Nothing
reads across schemas on the server; the client stitches the answers.

| Module | Reads | Writes | Admin-read policy (new) |
| --- | --- | --- | --- |
| `workspaces` | `users` (no RLS), `workspaces`, `members`, `invitations` (counted) | nothing | `workspaces.workspaces`, `workspaces.members`, `workspaces.invitations` |
| `identity` | `accounts`, `provider_identities`, `passkeys` (counted), `sessions` (counted) | `accounts.disabled_at`, `sessions.ended_at`, `refresh_families.revoked_at`, `audit_events` | none (no RLS on `identity`) |
| `connectors` | `connections`, `streams` (never `credentials`, cursors, the Gmail index) | nothing | `connectors.connections`, `connectors.streams` |
| server (BFF) | — | deletes `bff.sessions` of a suspended account | — |

The admin-read policies are `FOR SELECT … USING
(current_setting('campbooks.admin_reads', true) = 'on')`, set only by
`kernel::db::begin_admin`. They are permissive and select-only: an admin
transaction sees every workspace's rows of those tables and can change none
of them. All admin reads, identity's included, run in a `begin_admin`
transaction. The server's pinned allowlist test lists exactly these five
tables; none is a content table. `invitations` is readable but only
counted (pending, not expired); its addresses never leave the server.

The ids that join the views: `workspaces.users.idp_subject` is the
identity account id (the token `sub`), returned as `account_id`;
`connectors.connections.workspace_id` and `granted_by` are the workspaces
module's ids.

## Operations and permissions

All under the `admin` security scheme (scope `admin:access`), with
`x-campbooks-admin` naming the permission; `super` implies each.

| Operation | Method and path | Permission |
| --- | --- | --- |
| `workspaces.admin_list_users` | `GET /admin/v1/workspaces/users?q=&after=&limit=` | `users.read` |
| `workspaces.admin_get_user` | `GET /admin/v1/workspaces/users/{user_id}` | `users.read` |
| `workspaces.admin_list_workspaces` | `GET /admin/v1/workspaces/workspaces?q=&after=&limit=` | `workspaces.read` |
| `workspaces.admin_get_workspace` | `GET /admin/v1/workspaces/workspaces/{workspace_id}` | `workspaces.read` |
| `identity.admin_get_account` | `GET /admin/v1/identity/accounts/{account_id}` | `users.read` |
| `identity.admin_suspend_account` | `POST /admin/v1/identity/accounts/{account_id}/suspend` | `users.suspend` |
| `identity.admin_unsuspend_account` | `POST /admin/v1/identity/accounts/{account_id}/unsuspend` | `users.suspend` |
| `connectors.admin_list_connections` | `GET /admin/v1/connectors/workspaces/{workspace_id}/connections` | `workspaces.read` |

- **Paging**: newest first by `(created_at, id)`, an opaque base64url
  cursor (`next`, passed back as `after`), `limit` 1–200 (default 50).
  `q` is a case-insensitive prefix of the name or address (`LIKE` wildcards
  escaped), at most 100 characters (`query_too_long`). Errors:
  `invalid_query`, `invalid_limit`, `invalid_cursor`. The `Admin<P>`
  extractor runs before parameter checks, so a non-administrator gets 404
  even for bad parameters.
- **Metadata only**: names and addresses people signed in with, roles,
  counts, states and times. No subject, snippet, body, document, cursor,
  token, credential or key. Each view's test asserts the secrets it has
  next to it (session hashes, passkey keys, sealed credentials, cursors,
  scopes) are absent.
- **Errors**: unknown ids answer 404 (as does a malformed id); a
  non-administrator answers 404 for everything (ADR 0015).

## Suspending

`identity::domain::suspension::suspend` in one transaction:

1. lock the account (`FOR UPDATE`); missing → 404, already suspended →
   no-op success (the answer is the account as it is);
2. `disabled_at = now`; end every live issuer session; revoke every
   refresh-token family (`revoke_reason = 'suspended'`);
3. `identity.audit_events` row `account_suspended` with the admin's account
   id and the counts;
4. publish `identity.account_suspended { account }` (instance-wide, v1).

Before it, the handler refuses an account with an admin profile:
`state.admin_profiles.permissions_of(account)` (the kernel port the admin
module implements) is not empty → 409 `account_is_admin`, localized, which
names `campbooks-server admin revoke`. No code outside the admin module
reads schema `admin`.

Every door then refuses (most of these checks existed, unreachable until
now, and are now tested end to end):

- provider sign-in → `/sign-in?error=account_suspended` (renamed from
  `account_disabled`, which nothing could trigger);
- the issuer session → ignored (`sessions::current` joins live accounts), so
  `/oauth2/authorize` opens the sign-in screen instead of single sign-on;
- authorization codes → `invalid_grant` (the profile lookup skips suspended
  accounts);
- refresh tokens → `invalid_grant` (families revoked, and `rotate` checks
  the account);
- passkey sign-in and native sign-in are not built yet on this branch; both
  will go through the same session and token code, which refuses suspended
  accounts.

The web BFF consumes the event (`bff.end_suspended_sessions`, queue
`events`) and deletes the account's `bff.sessions`: the browser's next
request answers `session_required`, the app sends the person to sign in,
and the sign-in screen says the account is suspended. Unsuspending clears
`disabled_at` and records `account_unsuspended`; nothing that was ended
comes back.

Access tokens already issued stay valid until they expire (ten minutes):
the API validates tokens without a database lookup. Native clients could
use one for that long; the web loses its session within seconds (the event
delivery). Follow-up: a revocation list or an account check in the token
verifier.

## Events

- Published: `identity.account_suspended` v1 `{ account: uuid }`, not
  workspace-scoped (in `openapi/events.json`).
- Consumed: by `bff.end_suspended_sessions` (server). The BFF is the
  server's own, which already composes the identity module; no module
  imports another.

## Client

- `modules/workspaces` contributes the **People** (`/admin/people`,
  `/admin/people/$userId`) and **Workspaces** (`/admin/workspaces`,
  `/admin/workspaces/$workspaceId`) panels. Their identity and connectors
  parts are components exported by those modules' `index.ts`
  (`AccountAdminPanel`, `WorkspaceConnectionsAdmin`) and handed in by the
  module manifest (`app/modules.ts`), the same injection the manifest
  already uses for the mailbox step, so `workspaces` imports neither.
- Lists: `TextField` search (debounced), `List`/`ListRow`, "Show more" with
  the cursor (`useInfiniteQuery`), loading skeleton, empty and error
  states. Details: `FactList`/`Fact` (status dots for suspension and
  connection health), `List` for workspaces and members.
- Suspend / Unsuspend: a danger button that opens `InlineConfirm` (the
  design system's confirmation: in place, focus on Cancel, Escape cancels)
  saying what happens ("signed out everywhere, can't sign in until
  unsuspended, nothing is deleted"); `account_is_admin` is shown as a
  banner. No modal: `lib/ui` has none and `InlineConfirm` is how every
  other risky action confirms.
- No new `lib/ui` component.

## Alternatives considered

1. **Where the admin views live.**
   - *(chosen)* Each owning module serves its own admin operations; the
     client stitches by id. Follows ADR 0002/0015; three small handlers per
     module instead of one, and two or three requests for a detail page.
   - A single "people" endpoint in `workspaces` that also returns identity's
     account and connectors' health through new kernel ports. One request
     per page, but three new ports used only by the admin section, and
     `workspaces` would know about suspension and mailboxes. Rejected: new
     machinery for a page an administrator opens rarely.
   - A server-level admin aggregator that queries every schema. Rejected by
     ADR 0015 (cross-schema SQL, rots as modules change).
2. **Where suspension is stored.**
   - *(chosen)* `identity.accounts.disabled_at`, which already existed and
     which every issuer door already checked. Identity owns sign-in, so the
     state lives where it is enforced.
   - A `suspended_at` on `workspaces.users`, enforced by the workspaces
     module's membership port on every API call. It would also cut off
     live access tokens at once, but sign-in would still succeed (identity
     knows nothing of it), every request would pay a lookup, and an
     external identity provider's users would be "suspended" only inside
     workspaces. Rejected; the ten-minute token window is a follow-up.
3. **Ending web sessions.**
   - *(chosen)* A domain event consumed by the BFF in-process: one handler,
     transactional with the suspension, no HTTP call to ourselves.
   - OIDC back-channel logout from the in-process issuer to the BFF's
     `/bff/backchannel-logout`: standard, but the issuer would have to mint
     logout tokens and call its own server over HTTP. More machinery for the
     same effect; worth it only for third-party relying parties, which do
     not exist yet.
   - Nothing: web sessions end at the next refresh (under ten minutes).
     Rejected: suspending should take effect now.
4. **Confirmation.** `InlineConfirm` (chosen, existing) versus a new modal
   dialog component (story, tests, focus trap) for this one page. The
   brief asked for a "confirm dialog"; the design system's answer to a
   risky action is the inline confirmation, which also explains the effect.
5. **Paging.** Keyset cursor (chosen: stable while people sign up) versus
   offset paging (simpler, but pages shift as rows arrive and get slower).

**Does it copy the old Rails app?** No. The Rails app had `/admin/users`
listing `User` rows with an `admin` boolean and no suspension; its admin
pages read every table directly. This slice keeps ADR 0015's split (each
module, metadata only, admin power unreachable from the catalogue) and adds
a suspension the Rails app never had.
