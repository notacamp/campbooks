# Design

## Context

Every `SECURITY DEFINER` function of the database comes from two changes:
the kernel's rate-limit functions (`20261021060000`, body replaced by
`20261021070100`) and the BFF's fifteen (`20261021070000`). Each was owned
by the login that applied its migration. In production that login is the
bundled Postgres's `POSTGRES_USER`, a superuser; the server queries as a
separate login, a member of the runtime role `campbooks_app` only.

Data ownership is unchanged: the kernel owns schema `kernel`, the server's
BFF owns schema `bff`, and each migration touches only its own. No event is
published or consumed anew, no endpoint is added or changed, no AI is used.

## Every definer function, its callers, and what it runs as

Found by `grep -rn "SECURITY DEFINER" migrations/` (17 functions) and every
call site in the crates. Every caller runs as the runtime role
(`campbooks_app`), on the application's pool or a job's `ctx.pool()`.

| Function | Callers | Before (runs as) | Now (runs as) |
|---|---|---|---|
| `kernel.take_rate_limit` | `kernel::rate_limit::RateLimits::check` (sign-in, token, registration and the API's per-caller limits), every request | the migration login (superuser in production) | `campbooks_definer` |
| `kernel.prune_rate_limits` | job `kernel.prune_rate_limits` (queue `maintenance`) | as above | `campbooks_definer` |
| `bff.start_login`, `bff.take_login` | `GET /bff/login`, `GET /bff/callback` | as above | `campbooks_definer` |
| `bff.create_session`, `bff.end_session` | the callback (a new session; the browser's earlier one ends), logout, a copied cookie, a refused refresh, scopes missing | as above | `campbooks_definer` |
| `bff.find_session`, `bff.retired_session`, `bff.touch_session`, `bff.mark_session_delivered`, `bff.rotate_session`, `bff.reissue_session` | every cookie request (`/bff/session`, `/bff/api/*`, `/bff/admin/*`, `/bff/logout`) | as above | `campbooks_definer` |
| `bff.lock_session_tokens`, `bff.store_session_tokens` | the single-flight token refresh | as above | `campbooks_definer` |
| `bff.end_account_sessions` | back-channel logout; handlers `bff.end_signed_out_sessions` and `bff.end_suspended_sessions` (queue `events`) | as above | `campbooks_definer` |
| `bff.prune_sessions`, `bff.prune_logins` | job `bff.prune_sessions` (queue `maintenance`, every 15 minutes) | as above | `campbooks_definer` |

The admin API, the admin CLI and `xtask` call none of them. Tests call them
as the runtime role (the BFF's end-to-end tests, the rate-limit tests, the
new ones below) and inspect the tables as the owner.

What `campbooks_definer` needs, statement by statement:

| Table | Privileges | Why |
|---|---|---|
| `kernel.rate_limits` | `SELECT, INSERT, UPDATE, DELETE` | `INSERT … ON CONFLICT DO UPDATE … RETURNING`; the prune's `SELECT` and `DELETE` |
| `bff.sessions` | `SELECT, INSERT, UPDATE, DELETE` | create; find; touch, deliver, rotate, reissue, store tokens (`UPDATE`); `FOR UPDATE` locks; end, account end, prune |
| `bff.login_transactions` | `SELECT, INSERT, DELETE` | start; take (`DELETE … RETURNING`); prune. Never updated |
| `bff.retired_tokens` | `SELECT, INSERT` | retired on rotate and reissue; read by find-retired and end. Deleted only by the foreign key's cascade, which Postgres runs as the table's owner |

Plus `USAGE` on schemas `kernel` and `bff`. Checked on a throwaway database:
every function called as the runtime role, the foreign key's check and
cascade included.

## Decisions

### One owner role, not one per schema

`campbooks_definer` owns the functions of both schemas. A role per schema
(`kernel`, `bff`) would keep a bug in a BFF function body from reaching the
rate-limit counters and the other way round, but the bodies are fixed
statements with no dynamic SQL, so a body can only reach another table by
being changed, and a change is reviewed. Against that, every role is
cluster-wide: two roles mean two creations, two administrator statements on
installations that cannot create roles, and two sets of tests. One role,
with its exact privileges pinned by a test, keeps the reach visible: a later
migration that grants it more fails that test until the list is updated.

### Row-level security: a policy for the owner, the tables keep theirs

The function owner is no longer the table owner, so the BFF's tables, whose
row-level security is enabled but not forced and has no policy, would hide
every row from the functions. Options:

1. **Give `campbooks_definer` the tables.** The owner bypasses unforced
   row-level security. But an owner may also alter, drop or disable
   row-level security on its tables and grant them to anyone; a later
   migration altering these tables would need the migration login to be a
   member of the role; and handing a table over needs the role to be allowed
   to create in the schema.
2. **Chosen: a policy for `campbooks_definer` only**
   (`CREATE POLICY definer_functions … TO campbooks_definer USING (true)
   WITH CHECK (true)`), row-level security still enabled, the tables still
   the migration login's. The role can do on them exactly what its
   privileges allow, and nothing else; every other role, the runtime role
   included, still sees no row even if given a privilege by mistake.

`kernel.rate_limits` needs nothing new: its row-level security is forced
with `kernel_scope_only` (all roles), which the functions satisfy by turning
the kernel scope on for their own statement, as before.

### The role is created by the migration, or the migration stops

Roles belong to the cluster, so the kernel migration creates the role once
(`IF NOT EXISTS`, tolerating losing the race to a test database migrating in
parallel) and checks one created beforehand is as narrow (no login, no
privileged attribute, no membership). A superuser may create it, and so may
a login with `CREATEROLE` (the database owner on most managed services). To
hand a function over, a login that is not a superuser must be able to
`SET ROLE campbooks_definer` (it grants the role to itself: a role's creator
holds its admin option) and the role must be allowed to create in the
function's schema (lent for the hand-over, revoked in the same
transaction). Replacing a function later (`CREATE OR REPLACE`) needs the
same membership, which stays.

A login that can do neither **fails the migration** with a message naming
the two statements an administrator runs once
(`CREATE ROLE campbooks_definer NOLOGIN NOINHERIT; GRANT campbooks_definer
TO <login>;`), and the server starts once they are run. Degrading (leaving
the functions with the login) was rejected: a migration runs once, so such
an installation would stay in a different shape from every other one
forever, and silently (sqlx does not show a migration's warnings). Those
installations already needed an administrator to create `campbooks_app`
(`deploy/README.md`), which now lists both statements. A superuser never
reaches this branch, so the bundled setup needs nothing.

Checked on throwaway databases: a superuser login; a database owner with
`CREATEROLE` creating the role; a plain database owner and a second
`CREATEROLE` login when the role already exists (each stops with the
message, then applies once an administrator has granted the role). In
every case all seventeen functions end up owned by `campbooks_definer`.

### Enforcement: the database test, not `xtask`

`definer_functions_belong_to_the_nologin_owner_role` reads `pg_proc` after
every migration: every `SECURITY DEFINER` function, in any schema, belongs
to `campbooks_definer`, which is no superuser and cannot log in. It also
pins the role's attributes, that it is a member of no role and the runtime
role is not a member of it, that it owns nothing else, and its exact table,
column, schema, function and database privileges. A later migration adding a
definer function without `ALTER FUNCTION … OWNER TO campbooks_definer`
fails it, naming the function. An `xtask` rule would read SQL text and miss
a function created another way; the catalogue misses nothing. So only the
test.

### Input checks (items 3 and 4)

- `bff.start_login` checks the landing exactly as the handler's
  `return_to`: at most 2 048 characters, a leading `/` and never `//`,
  visible ASCII only (browsers drop tabs and newlines, so `/<tab>/host`
  would be `//host`), no backslash (read as `/`), none of `"`, `'`, `<`, `>`
  and the backtick (the landing page writes it into HTML), and not `/bff`
  or below.
- `bff.touch_session` refuses a missing time or idle deadline, or a deadline
  not after the use: a missing deadline used to make `least()` keep the
  session until its absolute end.
- `bff.store_session_tokens` refuses a missing or empty sealed access or
  refresh token and a missing deadline (a refresh always stores the new
  refresh token or the one it used).
- `bff.create_session` also refuses an idle deadline not after the start and
  a session longer than 366 days; `CAMPBOOKS_BFF__ABSOLUTE_TIMEOUT_HOURS`
  is bounded the same (at most 8 784), so a configured instance never asks
  for one.

All with `22023`, as before. The handlers already send nothing these
refuse; the checks matter only to SQL reaching the functions directly.

### `xtask` (item 2)

The pinned-path rule matched `pg_temp\b`, and `$` is legal inside an
identifier, so `pg_temp$x` (another schema) passed. The `regex` crate has no
look-ahead, so `pg_temp([\w$]*)` captures what follows and a non-empty
capture is refused (the effect of `pg_temp(?![\w$])`). Separately, string
literals are dropped before the rule reads a migration, so
`set_config('search_path', …)` went unseen: a string literal naming
`search_path`, in any case, is now refused.

### Coverage (item 6)

`has_table_privilege` does not see a grant on a column. The kernel/BFF
coverage query, the BFF tables' and the rate-limit counters' checks and the
instance-only table's write check also ask `has_any_column_privilege`; a
test grants one column of `bff.sessions` and sees the table reported open.

### The callback revokes the earlier session's refresh token (item 5)

`GET /bff/callback` ended the browser's earlier session but dropped the
sealed refresh token `bff.end_session` returned, leaving it valid at the
identity provider until it expired. It now ends it through `Inner::retire`,
which revokes it (failures logged, as for every ended session) and leaves
the identity provider's session alone, since the sign-in that just finished
may have gone straight through it.

### A forged session's display (item 7, noted)

The runtime role can create a session for a cookie of its own choosing under
any subject, name and email (`create_session` cannot know who signed in).
`/bff/session` would then display that name and email, while every API call
through the BFF still runs on the sealed tokens, which only the server can
seal: the session can do nothing as the person it names. Binding the
displayed profile to the sealed ID token would close the display gap too; it
is left for later.

## Can the migration login stop being a superuser? (campbooks-cloud#174)

After this change nothing the server runs needs one. The migration login
needs to own the schemas and tables (the database owner does), to create or
take `campbooks_definer` and `campbooks_app` (`CREATEROLE`, or an
administrator once), and to create the trusted extensions `unaccent` and
`pg_trgm` (the database owner may). On the hosted cloud this would mean a
dedicated owner login with `CREATEROLE` holding the database (ownership of
every existing object reassigned to it, `REASSIGN OWNED`), the bundled
superuser kept for break-glass only. That is an operations change for
campbooks-cloud#174, not part of this one; **production needs no cloud-side
change for this one**: its migration login is the superuser, which creates
the role and hands the functions over, and its runtime login is a member of
`campbooks_app` only.

## Threat model

A bug in a definer function's body now runs as a role that cannot log in,
has no privileged attribute, owns no table and may only read and write the
four tables above: it cannot read workspace data, change a schema, create
roles or reach another database. What it could do with its privileges is
what the function's own statements already do. The boundary of #528 is
unchanged: the runtime role still reaches the four tables only through the
functions. With the single-URL setup (a privileged login switching to the
runtime role with `SET ROLE`), a query able to `RESET ROLE` still escapes,
as before (`kernel::db::inspect_role` reports it).

## Risks

- **Background work that could stop.** The rate limits (every sign-in, token
  and API request), the prune jobs (`kernel.prune_rate_limits`,
  `bff.prune_sessions`), the account handlers (`bff.end_signed_out_sessions`,
  `bff.end_suspended_sessions`) and every web request go through these
  functions. A privilege the role lacked would fail them with `permission
  denied`, loudly, not silently. Every function was called as the runtime
  role on a throwaway database after the migration, and the server's tests
  (which run the app and the worker as the runtime role) cover each caller.
- **Deploy order.** The previous build keeps working after the migration:
  same functions, signatures and grants, and the stricter checks refuse
  nothing it sends (its absolute timeout defaults to 30 days). The API and
  the worker may be replaced one after the other.
- **An installation that cannot create roles** stops at the upgrade until an
  administrator runs two statements (above, and in `deploy/README.md`).
- **Another Campbooks database on the same server** shares the role; each
  database's migration grants it that database's privileges only.
- **Lock timeout.** `CREATE POLICY` takes an `ACCESS EXCLUSIVE` lock on the
  three BFF tables; `lock_timeout = 5s` fails the migration (and the start)
  on a busy table rather than queueing every request, and the next start
  tries again.

## Rollback

sqlx has no down migrations and refuses to start with applied migrations it
does not know. **Ops step: before redeploying an image older than this
change, run this once as the migration login.** It gives every definer
function back to that login, restores the four replaced bodies of
`20261021070000`, removes the role's privileges and policies, and forgets
both migrations, so the older image starts and a later upgrade applies them
again. No row changes and nobody is signed out. The last statement drops the
role, and fails harmlessly (outside the transaction) when another database
of the server still holds anything of it, or when the login may not drop
roles.

Checked against a database migrated only to the base (`20261021070100`), on
the catalogue (`pg_dump` of the local client is older than the server):
every function's owner, language, volatility, settings, grants and body
digest, every table's owner, row-level security and grants, column grants,
policies, the schemas' owners and grants, and `_sqlx_migrations` are
identical after the rollback, as a superuser and as a plain database owner;
re-upgrading then applies both migrations cleanly.

```sql
-- Roll back campbooks-cloud#214 BEFORE redeploying an image older than it.
-- Run once, as the migration login (the login that owned the functions
-- before; a superuser in the bundled setup). Gives every definer function
-- back to that login, restores the bodies of 20261021070000, removes what
-- the owner role was granted and its policies, and forgets both migrations,
-- so the older image starts and a later upgrade applies them again. Nobody
-- is signed out; no row changes.
BEGIN;
SET LOCAL lock_timeout = '5s';
ALTER FUNCTION kernel.take_rate_limit(bytea, double precision, integer) OWNER TO CURRENT_USER;
ALTER FUNCTION kernel.prune_rate_limits(integer) OWNER TO CURRENT_USER;
ALTER FUNCTION bff.create_session(uuid, bytea, text, text, text, text, bytea, timestamptz, bytea,
                                  bytea, text, timestamptz, timestamptz, timestamptz, text)
    OWNER TO CURRENT_USER;
ALTER FUNCTION bff.find_session(bytea) OWNER TO CURRENT_USER;
ALTER FUNCTION bff.touch_session(bytea, timestamptz, timestamptz) OWNER TO CURRENT_USER;
ALTER FUNCTION bff.mark_session_delivered(bytea) OWNER TO CURRENT_USER;
ALTER FUNCTION bff.rotate_session(bytea, bytea, timestamptz) OWNER TO CURRENT_USER;
ALTER FUNCTION bff.reissue_session(bytea, bytea, timestamptz, timestamptz) OWNER TO CURRENT_USER;
ALTER FUNCTION bff.retired_session(bytea) OWNER TO CURRENT_USER;
ALTER FUNCTION bff.end_session(bytea) OWNER TO CURRENT_USER;
ALTER FUNCTION bff.lock_session_tokens(bytea) OWNER TO CURRENT_USER;
ALTER FUNCTION bff.store_session_tokens(bytea, bytea, timestamptz, bytea) OWNER TO CURRENT_USER;
ALTER FUNCTION bff.end_account_sessions(text, text) OWNER TO CURRENT_USER;
ALTER FUNCTION bff.prune_sessions(integer) OWNER TO CURRENT_USER;
ALTER FUNCTION bff.start_login(bytea, bytea, bytea, bytea, text, timestamptz, timestamptz, text)
    OWNER TO CURRENT_USER;
ALTER FUNCTION bff.take_login(bytea) OWNER TO CURRENT_USER;
ALTER FUNCTION bff.prune_logins(integer) OWNER TO CURRENT_USER;
-- The bodies of 20261021070000.
CREATE OR REPLACE FUNCTION bff.create_session(new_id uuid, new_token bytea, account text,
                                   idp_session text, display_name text, display_email text,
                                   sealed_access bytea, access_until timestamptz,
                                   sealed_refresh bytea, sealed_id bytea, agent text,
                                   started timestamptz, idle_until timestamptz,
                                   ends timestamptz, asked text)
    RETURNS void
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
BEGIN
    IF new_id IS NULL OR new_token IS NULL OR octet_length(new_token) <> 32 THEN
        RAISE EXCEPTION 'a session has an id and a 32-byte token hash' USING ERRCODE = '22023';
    END IF;
    IF account IS NULL OR btrim(account) = '' THEN
        RAISE EXCEPTION 'a session belongs to an account' USING ERRCODE = '22023';
    END IF;
    IF sealed_access IS NULL OR sealed_id IS NULL OR access_until IS NULL THEN
        RAISE EXCEPTION 'a session holds sealed tokens' USING ERRCODE = '22023';
    END IF;
    IF started IS NULL OR idle_until IS NULL OR ends IS NULL
       OR NOT (started < ends AND idle_until <= ends) THEN
        RAISE EXCEPTION 'a session ends after it starts, idle at the latest when it ends'
            USING ERRCODE = '22023';
    END IF;
    INSERT INTO bff.sessions
        (id, token_hash, rotated_at, subject, idp_session_id, name, email,
         access_token, access_expires_at, refresh_token, id_token, user_agent,
         created_at, last_seen_at, idle_expires_at, expires_at, scopes)
    VALUES (new_id, new_token, started, account, idp_session, display_name, display_email,
            sealed_access, access_until, sealed_refresh, sealed_id, agent,
            started, started, idle_until, ends, asked);
END
$$;
CREATE OR REPLACE FUNCTION bff.touch_session(presented bytea, seen timestamptz, idle_until timestamptz)
    RETURNS void
    LANGUAGE sql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
    UPDATE bff.sessions s
       SET last_seen_at = seen, idle_expires_at = least(idle_until, s.expires_at)
     WHERE (s.token_hash = presented OR s.previous_token_hash = presented)
       AND s.idle_expires_at > now()
       AND s.expires_at > now()
$$;
CREATE OR REPLACE FUNCTION bff.store_session_tokens(presented bytea, sealed_access bytea,
                                         access_until timestamptz, sealed_refresh bytea)
    RETURNS void
    LANGUAGE sql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
    UPDATE bff.sessions s
       SET access_token = sealed_access, access_expires_at = access_until,
           refresh_token = sealed_refresh
     WHERE s.token_hash = presented OR s.previous_token_hash = presented
$$;
CREATE OR REPLACE FUNCTION bff.start_login(state bytea, browser bytea, sealed_nonce bytea,
                                sealed_verifier bytea, landing text, started timestamptz,
                                ends timestamptz, asked text)
    RETURNS void
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
BEGIN
    IF state IS NULL OR octet_length(state) <> 32
       OR browser IS NULL OR octet_length(browser) <> 32 THEN
        RAISE EXCEPTION 'a sign-in has 32-byte state and browser hashes' USING ERRCODE = '22023';
    END IF;
    IF sealed_nonce IS NULL OR sealed_verifier IS NULL THEN
        RAISE EXCEPTION 'a sign-in holds a sealed nonce and verifier' USING ERRCODE = '22023';
    END IF;
    IF landing IS NULL OR left(landing, 1) <> '/' THEN
        RAISE EXCEPTION 'a sign-in lands on a same-origin path' USING ERRCODE = '22023';
    END IF;
    IF started IS NULL OR ends IS NULL
       OR NOT (started < ends AND ends - started <= interval '1 hour') THEN
        RAISE EXCEPTION 'a sign-in lasts more than nothing and at most an hour'
            USING ERRCODE = '22023';
    END IF;
    INSERT INTO bff.login_transactions
        (state_hash, browser_hash, nonce, code_verifier, return_to, created_at, expires_at,
         scopes)
    VALUES (state, browser, sealed_nonce, sealed_verifier, landing, started, ends, asked);
END
$$;
DROP POLICY definer_functions ON bff.sessions;
DROP POLICY definer_functions ON bff.login_transactions;
DROP POLICY definer_functions ON bff.retired_tokens;
REVOKE ALL ON bff.sessions, bff.login_transactions, bff.retired_tokens FROM campbooks_definer;
REVOKE ALL ON kernel.rate_limits FROM campbooks_definer;
REVOKE ALL ON SCHEMA bff, kernel FROM campbooks_definer;
DELETE FROM _sqlx_migrations WHERE version IN (20261021080000, 20261021080100);
COMMIT;
-- The role belongs to the cluster: drop it only when no database still
-- holds anything of it (another Campbooks database on the same server may).
-- A failure here changes nothing and can be ignored.
DROP ROLE IF EXISTS campbooks_definer;
```

The previous build runs on either side of this change, so no process needs
stopping first; an image of this change started after the rollback applies
both migrations again.
