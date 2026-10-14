# Tasks

## 1. Data and policy

- [x] 1.1 Migration `workspaces__signups`: `signup_requests` (RLS: own row; admin-read SELECT/UPDATE), `signup_codes`, grants
- [x] 1.2 `signup::admit` (approval → request filing, approved/rejected/pending outcomes; code → environment codes or a minted digest), `signup::redeem` inside workspace creation; `signup_rejected` and validation messages in four locales
- [x] 1.3 Kernel: the `code` policy needs no environment code
- [x] 1.4 Security log kinds `signup_approved`, `signup_rejected`, `signup_code_redeemed`

## 2. API

- [x] 2.1 `GET /api/v1/signup` (`workspaces.get_signup`)
- [x] 2.2 Admin: list / approve / reject requests; list / mint / revoke codes; paging; permission gate tests, idempotence, concurrency test, no plaintext in lists
- [x] 2.3 Server: pinned admin-read allowlist gains `workspaces.signup_requests`; `openapi/openapi.json` and `openapi/admin.json` regenerated; `.sqlx` regenerated

## 3. Client

- [x] 3.1 Admin → Signups panel (requests, codes) with confirmations and empty/loading/error states, four locales
- [x] 3.2 CreateWorkspace: waiting / approved / not approved views
- [x] 3.3 Tests and a recording scenario (`admin-signups`)

## 4. Docs

- [x] 4.1 CHANGELOG, `deploy/README.md` (signup policy and codes)
