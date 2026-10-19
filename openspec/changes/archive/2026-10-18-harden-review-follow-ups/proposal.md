## Why

The security reviews of #527, #529 and #539 left non-blocking findings in
campbooks-cloud#213. Items 1, 2, 5 and 11 shipped with #529. This change does
the rest that is code or documentation:

- Idempotency keys are shared by one person's apps (item 3).
- No guard requires `secret_once` on answers that hold secrets (item 4).
- The self-hosting docs overstate what the database separation protects
  (item 6).
- The kernel coverage test misses views and open policies (item 7).
- A runtime login may hold the definer role (item 8), and the migration
  login may lack its `USAGE` (item 12).
- Clock skew turns a session touch into a 500 (item 9).
- Lock waits at deploy are undocumented and unbounded (items 10 and 16).
- An invalid index can survive `cargo sqlx migrate run` (item 14).
- There is no PostgreSQL version check (item 15).

## What Changes

- **Per-app idempotency keys (item 3).** A person's key space is
  `user:<length>:<subject>|azp:<client>`. The web app and every app acting
  for the same person no longer share keys. Keys stored before the upgrade
  stop matching: a retry that spans the upgrade runs again (within 24 hours,
  for keys made just before it).
- **Secrets in answers (item 4).**
  - `cargo xtask arch` reads every schema a mutating operation answers below
    400 with, at any depth. A field named like a secret (`secret`, `token`,
    `ticket`, `password`, `*url`) needs `secret_once` or a listed reason.
  - The token endpoint (`identity.token`) now declares `secret_once`.
  - The idempotency layer withholds any answer below 400 of a `secret_once`
    operation, not only a 2xx. A redirect's `Location` is never kept.
- **Coverage (item 7).** The kernel and BFF coverage test now includes
  views and materialized views, which need no grant to the runtime role. A
  permissive policy for the runtime role (or everyone) that admits every row
  no longer counts as protection.
- **The definer role (items 8 and 12).**
  - The server refuses to start when the role it queries as is a member of
    `campbooks_definer`.
  - A migration refuses a `campbooks_app` that is one.
  - The migration login must hold the role with `INHERIT`; a migration
    grants that itself when PostgreSQL 16+ allows it, else names the
    statement.
- **Clock skew (item 9).** The BFF records a use only with an idle deadline
  after it, so a server clock ahead of the database's never makes
  `bff.touch_session` fail the request.
- **Startup (items 15 and 16).**
  - The server refuses a PostgreSQL older than 15, before migrating.
  - The migration connection waits at most a minute for a lock where a
    migration sets no limit of its own. It does this after taking the
    migration lock, so a startup behind a long transaction fails and retries
    instead of hanging.
- **Job key indexes (item 14).** A new migration rebuilds the queue's
  per-workspace key indexes when they are missing or invalid.
- **Docs (items 6 and 10).** The self-hosting docs state what the database
  separation protects and when. They also cover PostgreSQL 15+ (17
  recommended), the definer role's grants, and how a migration behaves
  behind a long transaction.

## Impact

- Specs:
  - access (idempotency keys, kernel table coverage, definer role);
  - operation-catalogue (declared operations, idempotent operations);
  - workspaces (runtime login);
  - web-bff (session store);
  - events-and-jobs (the queue's key indexes).
- Code:
  - kernel `idempotency`, `db` (`RoleReport::holds_definer_rights`,
    `server_version_num`);
  - server `main` (startup checks, migration lock timeout) and `bff`
    (`touch_until`);
  - identity `protocol::token`;
  - xtask `arch` (`check_answer_field_names`).
- Migrations: `20261022030000_kernel__definer_membership_checks`,
  `20261022030100_kernel__job_key_indexes_valid`.
- Not done: the CHANGELOG line of #527 stays as released; the docs now
  qualify it.
