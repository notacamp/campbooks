# Proposal

## Why

The review of notacamp/campbooks#528 (campbooks-cloud#214) found that every
`SECURITY DEFINER` function, the kernel's two rate-limit functions (#527)
and the BFF's fifteen (#528), is owned by the login that applied the
migrations. A definer function runs with its owner's rights, and on the
bundled Postgres that login is the superuser (`POSTGRES_USER`): a bug in a
function body would run with every right in the cluster. The functions'
bodies are fixed statements with a pinned path, so this needs a mistake in
a later change, but nothing structural stopped one from reaching
everything.

The same review left six small follow-ups (items 2 to 7 of the issue); five
come along, item 7 as a design note.

Modules touched: the kernel (a migration creating the owner role and handing
over its two functions, doc comments), the server's web BFF (a migration
handing over its fifteen functions with stricter input checks, the callback,
the absolute timeout's bound), the server's tests and `xtask` (the pinned
`search_path` rule). No new module and no new cross-module dependency.

## What Changes

- **A dedicated owner role.** `campbooks_definer`: `NOLOGIN`, no superuser,
  `BYPASSRLS`, `CREATEDB`, `CREATEROLE` or `REPLICATION`, a member of no
  role. It owns every `SECURITY DEFINER` function and nothing else, and holds
  exactly the privileges the functions' statements need: `USAGE` on schemas
  `kernel` and `bff`; `SELECT, INSERT, UPDATE, DELETE` on
  `kernel.rate_limits` and `bff.sessions`; `SELECT, INSERT, DELETE` on
  `bff.login_transactions`; `SELECT, INSERT` on `bff.retired_tokens`. The
  migration creates it (a superuser or a login with `CREATEROLE` may), or
  stops with the two statements an administrator must run once.
- **Row-level security unchanged in effect.** The rate-limit counters keep
  their forced kernel-scope policy, which the functions satisfy as before.
  The BFF's tables keep their owner and row-level security (not forced) and
  gain one policy, for `campbooks_definer` only; every other role, the
  runtime role included, still sees nothing.
- **Grants unchanged.** Each function stays executable by the runtime role
  only (asserted again after the hand-over).
- **Enforced by a test.** A server test fails naming any definer function
  owned by another role, a superuser or a login, and pins the role's
  attributes, memberships, ownerships and privileges. A later migration
  adding a definer function must hand it over.
- **Follow-ups (campbooks-cloud#214).**
  - `cargo xtask arch` refuses `pg_temp` followed by an identifier character
    (`pg_temp$x`), and a string naming `search_path` (`set_config`) (2).
  - `bff.start_login` checks the landing as the handler does: no `//`, no
    backslash, visible ASCII only, none of `"'<>` and the backtick, not the
    BFF's paths, at most 2 048 characters (3).
  - `bff.touch_session` refuses a missing time or deadline, or a deadline
    not after the use; `bff.store_session_tokens` refuses a missing or empty
    sealed token or a missing deadline; `bff.create_session` refuses a
    session longer than 366 days or an idle deadline not after its start.
    `CAMPBOOKS_BFF__ABSOLUTE_TIMEOUT_HOURS` must be at most 8 784 (4).
  - Signing in again in the same browser revokes the earlier session's
    refresh token (5).
  - The coverage tests also see column-level grants
    (`has_any_column_privilege`) (6).
  - The design notes what a forged session can display (7).
- **Migrations** `20261021080000_kernel__definer_owner_role.sql` and
  `20261021080100_bff__definer_owner_and_input_checks.sql`, each one
  transaction with a lock timeout.

## Non-goals

- Making the migration login itself a non-superuser on the hosted cloud
  (campbooks-cloud#174, the break-glass role): this change makes that
  possible, it does not do it.
- One owner role per schema (see design).
- Binding the displayed profile to the sealed ID token (item 7): noted only.

## Impact

- **Nobody is signed out** and no row changes. The previous build keeps
  working after the migration (same functions, same grants), so the API and
  the worker may be replaced one after the other.
- An installation whose migration login is neither a superuser nor allowed
  to create roles stops at the upgrade until an administrator runs
  `CREATE ROLE campbooks_definer NOLOGIN NOINHERIT; GRANT campbooks_definer
  TO <migration login>;`. The bundled Postgres needs nothing.
- An instance configured with `CAMPBOOKS_BFF__ABSOLUTE_TIMEOUT_HOURS` over
  8 784 now refuses to start, naming the setting.
