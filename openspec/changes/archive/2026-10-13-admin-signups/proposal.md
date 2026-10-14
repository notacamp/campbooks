# Proposal

## Why

The `approval` signup policy records a security event and refuses; nobody
can act on it. The `code` policy compares against codes in an environment
variable that are reusable by anyone who learns one and cannot be revoked
without a redeploy. Administrators need to see who is waiting, let them in
or turn them down, and hand out codes that work once. This is slice S1 of the
admin epic (ADR 0015).

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/97
(epic https://github.com/notacamp/campbooks-cloud/issues/95).

## What Changes

- **`workspaces` module** (it owns the signup policy):
  - `workspaces.signup_requests` (one per user: pending / approved /
    rejected, who decided and when, an optional reason). A refused attempt
    under `approval` files one pending request, once. An approved user passes
    the policy; a rejected user is refused with the new problem code
    `signup_rejected`.
  - `workspaces.signup_codes` (SHA-256 digest only, label, `max_uses`
    default 1, `uses`, optional expiry, revocation). Redeemed by one atomic
    update in the transaction that creates the workspace. Environment codes
    keep working exactly as before (the legacy path) and the `code` policy no
    longer requires one at startup.
  - Admin operations under `/admin/v1/workspaces/`: list signup requests
    (status filter, cursor paging), approve, reject (optional reason), list
    codes, mint a code (plaintext in that response only), revoke a code. All
    need `signups.review`.
  - Member operation `GET /api/v1/signup`: the instance's policy and the
    caller's own request status.
  - Approve and reject write the security log; redeeming a code writes the
    new workspace's security log.
- **Client**: Admin → Signups (Requests, Codes), and the create-workspace
  screen's "waiting", "approved, continue" and "not approved" views.
- **Kernel**: the startup check that `CAMPBOOKS_SIGNUP__POLICY=code` needs a
  code in `CAMPBOOKS_SIGNUP__CODES` is removed (minted codes need no
  setting). `workspaces.signup_requests` joins the pinned admin-read allowlist.

## Non-goals

- Emailing the person when their request is decided (there is no
  notifications module yet): they see the new state the next time they open
  the app. Follow-up.
- Codes that grant anything beyond creating a first workspace (roles, plans,
  workspace invitations).
- Editing a minted code (label, uses, expiry): mint a new one and revoke the
  old.
- Listing or searching users (S2), workspaces (S2) and jobs (S3).
- Changing the policy from the UI: it stays `CAMPBOOKS_SIGNUP__POLICY`.
