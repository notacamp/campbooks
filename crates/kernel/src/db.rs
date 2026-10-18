//! Database conventions every module relies on (ADR 0011): the role queries
//! run as, and the workspace a transaction works in.
//!
//! **Row-level security.** Workspace-owned tables carry Postgres policies
//! that admit only the rows of the workspace in the transaction-local setting
//! [`WORKSPACE_SETTING`]. Superusers and `BYPASSRLS` roles skip policies, and
//! the database user a self-hoster connects with is often a superuser, so the
//! server's pool switches every connection to a **runtime role**
//! ([`DEFAULT_RUNTIME_ROLE`], `CAMPBOOKS_DATABASE_ROLE`) that is subject to
//! them. Migrations run on their own connection; the kernel migration
//! creates the runtime role, and each migration grants it what its own
//! tables need.
//!
//! **Two logins (recommended).** `CAMPBOOKS_DATABASE_URL` is a dedicated
//! non-superuser `LOGIN` role that is a member of the runtime role (no
//! `SET ROLE` needed, nothing to `RESET ROLE` back to), and
//! `CAMPBOOKS_DATABASE_MIGRATION_URL` is the owner login that only runs
//! migrations. With a single privileged URL the `SET ROLE` switch keeps
//! working, with a warning ([`RoleVerdict::Escapable`]).
//!
//! **The workspace.** A request works in one workspace through
//! [`BeginFor::begin_for`]; a job's [`JobContext::tx`](crate::JobContext::tx)
//! sets the same setting. A transaction that sets no workspace sees no
//! workspace's rows, the kernel's included; the kernel's machinery works
//! across workspaces only in the explicit **kernel scope**
//! ([`begin_kernel`], the setting `campbooks.kernel_scope`).
//!
//! ```no_run
//! # async fn demo(state: campbooks_kernel::AppState, workspace: campbooks_kernel::tenancy::WorkspaceId) -> Result<(), sqlx::Error> {
//! use campbooks_kernel::db::BeginFor as _;
//!
//! let mut tx = state.db.begin_for(workspace).await?;
//! // queries here see only `workspace`'s rows
//! tx.commit().await
//! # }
//! ```

use std::{future::Future, time::Duration};

use sqlx::{
    Connection as _, PgConnection, PgPool, Postgres, Transaction,
    postgres::{PgConnectOptions, PgPoolOptions},
};

use crate::{admin::AdminGrant, tenancy::WorkspaceId};

/// The transaction-local Postgres setting that carries the current
/// workspace: `current_setting('campbooks.workspace_id', true)`. Policies
/// read it as `nullif(current_setting('campbooks.workspace_id', true), '')::uuid`
/// (a setting a connection has used before reads as `''` once unset).
pub const WORKSPACE_SETTING: &str = "campbooks.workspace_id";

/// The role created by the kernel migration and used at runtime unless
/// `CAMPBOOKS_DATABASE_ROLE` says otherwise: `NOLOGIN`, not a superuser, no
/// `BYPASSRLS`.
pub const DEFAULT_RUNTIME_ROLE: &str = "campbooks_app";

/// Longest one statement on a runtime connection may run
/// (`statement_timeout`) before Postgres cancels it: a request never waits
/// on the database for ever (campbooks-cloud#251).
pub const STATEMENT_TIMEOUT: Duration = Duration::from_secs(30);

/// Longest a runtime statement may wait for a lock (`lock_timeout`), for
/// example behind the sequencer's `FOR UPDATE` on `kernel.change_heads`.
pub const LOCK_TIMEOUT: Duration = Duration::from_secs(10);

/// Longest one statement may run in a job's own transaction: jobs work in
/// larger batches than requests, but never without a bound.
pub const JOB_STATEMENT_TIMEOUT: Duration = Duration::from_mins(5);

/// `options` whose connections start with [`STATEMENT_TIMEOUT`] and
/// [`LOCK_TIMEOUT`] (the startup `options` parameter, so they hold before
/// the first query and whatever role the connection switches to). The
/// runtime pool's connections; migrations connect without them.
pub fn with_session_limits(options: PgConnectOptions) -> PgConnectOptions {
    options.options([
        (
            "statement_timeout",
            format!("{}ms", STATEMENT_TIMEOUT.as_millis()),
        ),
        ("lock_timeout", format!("{}ms", LOCK_TIMEOUT.as_millis())),
    ])
}

/// Let the statements of the current transaction run for up to `limit`
/// (`statement_timeout`, until the transaction ends).
pub(crate) async fn allow_statements_for(
    conn: &mut PgConnection,
    limit: Duration,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT set_config('statement_timeout', $1, true)")
        .bind(format!("{}ms", limit.as_millis()))
        .execute(conn)
        .await?;
    Ok(())
}

/// Set [`WORKSPACE_SETTING`] to `workspace` until the current transaction
/// ends. Outside a transaction it would last for one statement only.
pub async fn set_workspace(conn: &mut PgConnection, workspace: &str) -> Result<(), sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT set_config($1, $2, true)",
        WORKSPACE_SETTING,
        workspace
    )
    .fetch_one(conn)
    .await?;
    Ok(())
}

/// Clear [`WORKSPACE_SETTING`] until the current transaction ends (or it is
/// set again): the transaction works in no workspace.
pub(crate) async fn clear_workspace(conn: &mut PgConnection) -> Result<(), sqlx::Error> {
    sqlx::query_scalar!("SELECT set_config($1, '', true)", WORKSPACE_SETTING)
        .fetch_one(conn)
        .await?;
    Ok(())
}

/// Opening a transaction that works in one workspace.
pub trait BeginFor {
    /// Begin a transaction with `campbooks.workspace_id` set to `workspace`
    /// (`SET LOCAL`), so row-level security admits that workspace's rows
    /// only.
    fn begin_for(
        &self,
        workspace: WorkspaceId,
    ) -> impl Future<Output = Result<Transaction<'static, Postgres>, sqlx::Error>> + Send;
}

impl BeginFor for PgPool {
    async fn begin_for(
        &self,
        workspace: WorkspaceId,
    ) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
        let mut tx = self.begin().await?;
        set_workspace(&mut tx, &workspace.to_string()).await?;
        Ok(tx)
    }
}

/// A savepoint inside `tx`: work that can be undone on its own (commit to
/// keep it, roll back or drop to undo it) without undoing the rest of `tx`.
/// It runs in `tx`, so it keeps `tx`'s workspace setting and locks.
pub async fn savepoint<'t>(
    tx: &'t mut Transaction<'_, Postgres>,
) -> Result<Transaction<'t, Postgres>, sqlx::Error> {
    (**tx).begin().await
}

/// The transaction-local Postgres setting that lets an administrator's
/// transaction read across workspaces, on the tables whose module added an
/// admin-read policy (metadata only, never content; ADR 0015). Only the
/// kernel names it: policies compare it with `'on'`, and
/// [`begin_admin`] is the only way to set it (`cargo xtask arch` rejects
/// the name anywhere else in Rust).
pub const ADMIN_READS_SETTING: &str = "campbooks.admin_reads";

/// Begin a transaction in which tables carrying an admin-read policy show
/// every workspace's rows (`campbooks.admin_reads = on`, `SET LOCAL`).
/// Requires the [`AdminGrant`] that only the
/// [`Admin`](crate::admin::Admin) extractor builds. Tables without such a
/// policy (content: messages, bodies, attachments, documents) stay
/// workspace-isolated.
pub async fn begin_admin(
    pool: &PgPool,
    grant: &AdminGrant,
) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
    tracing::debug!(
        permissions = ?grant.permissions().names(),
        "admin transaction reading across workspaces"
    );
    let mut tx = pool.begin().await?;
    sqlx::query_scalar!("SELECT set_config($1, 'on', true)", ADMIN_READS_SETTING)
        .fetch_one(&mut *tx)
        .await?;
    Ok(tx)
}

/// The transaction-local Postgres setting that lets a transaction work
/// across workspaces on the kernel's tables (the event log, the job queue,
/// the change feed, the blob catalogue) and on the module tables whose
/// maintenance policy admits it (campbooks-cloud#129). A transaction with
/// neither a workspace nor this scope sees no row of any table with
/// row-level security. Only the kernel names it (crate-private): policies
/// compare it with `'on'`, [`begin_kernel`] and
/// [`JobContext::kernel_tx`](crate::JobContext::kernel_tx) set it, and
/// `cargo xtask arch` rejects the setting, this constant's name and those
/// two functions' names anywhere else in Rust, except the reviewed
/// `[[access.kernel_scope]]` entries of `architecture.toml` (for the two
/// functions) and test code.
pub(crate) const KERNEL_SCOPE_SETTING: &str = "campbooks.kernel_scope";

/// Begin a transaction in the **kernel scope**
/// (`campbooks.kernel_scope = on`, `SET LOCAL`): the kernel's machinery
/// working across workspaces (workers claiming fairly, pruning, the blob
/// sweeper, replay, the admin's queue) and the few module maintenance paths
/// `architecture.toml` lists (a scheduler reading its index across
/// workspaces). Everything else works in one workspace
/// ([`BeginFor::begin_for`], a job's [`tx`](crate::JobContext::tx)).
pub async fn begin_kernel(pool: &PgPool) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    enter_kernel_scope(&mut tx).await?;
    Ok(tx)
}

/// The transaction-local Postgres setting that names the caller whose
/// idempotency keys a transaction reaches (`user:<subject>` or
/// `client:<client id>`; campbooks-cloud#210). `kernel.idempotency`'s
/// row-level security admits a key only to its caller, so a workspace
/// transaction, or one that set nothing, sees and writes no key. Only the
/// kernel names it (crate-private): [`begin_as_idempotency_caller`], called
/// by the idempotency layer alone, sets it, and `cargo xtask arch` rejects
/// the setting and this constant's name anywhere else in Rust.
pub(crate) const IDEMPOTENCY_CALLER_SETTING: &str = "campbooks.idempotency_caller";

/// Begin a transaction that reaches `caller`'s idempotency keys and nothing
/// else of the kernel's (no workspace, no kernel scope): the idempotency
/// layer's claim, store and release.
pub(crate) async fn begin_as_idempotency_caller(
    pool: &PgPool,
    caller: &str,
) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query_scalar!(
        "SELECT set_config($1, $2, true)",
        IDEMPOTENCY_CALLER_SETTING,
        caller
    )
    .fetch_one(&mut *tx)
    .await?;
    Ok(tx)
}

/// The transaction-local Postgres setting that a module's **maintenance
/// policies** trust, and nothing else: a scheduler's dispatcher reading and
/// leasing its own index across workspaces (`connectors.sync_due`,
/// `conversations.needs_backfills`; campbooks-cloud#196). The kernel's tables
/// do not admit it, so a transaction in this scope reaches no workspace's
/// events, jobs, changes or blobs: the dispatcher enqueues each run in the
/// run's workspace ([`set_workspace`] before the enqueue). Only the kernel
/// names it (crate-private); [`begin_maintenance`] and
/// [`JobContext::maintenance_tx`](crate::JobContext::maintenance_tx) set it,
/// and `cargo xtask arch` keeps those two to the reviewed
/// `[[access.maintenance_scope]]` entries of `architecture.toml`.
pub(crate) const MAINTENANCE_SCOPE_SETTING: &str = "campbooks.maintenance_scope";

/// Begin a transaction in the **maintenance scope**
/// (`campbooks.maintenance_scope = on`, `SET LOCAL`): only the module tables
/// whose maintenance policy admits it show rows across workspaces. Work on
/// one workspace's rows (an enqueue) sets that workspace first
/// ([`set_workspace`]).
pub async fn begin_maintenance(
    pool: &PgPool,
) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    enter_maintenance_scope(&mut tx).await?;
    Ok(tx)
}

/// Turn the maintenance scope on in the current transaction, until it ends.
pub(crate) async fn enter_maintenance_scope(conn: &mut PgConnection) -> Result<(), sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT set_config($1, 'on', true)",
        MAINTENANCE_SCOPE_SETTING
    )
    .fetch_one(conn)
    .await?;
    Ok(())
}

/// Turn the kernel scope on in the current transaction, until it ends or
/// [`leave_kernel_scope`].
pub(crate) async fn enter_kernel_scope(conn: &mut PgConnection) -> Result<(), sqlx::Error> {
    sqlx::query_scalar!("SELECT set_config($1, 'on', true)", KERNEL_SCOPE_SETTING)
        .fetch_one(conn)
        .await?;
    Ok(())
}

/// Turn the kernel scope off again in the current transaction.
pub(crate) async fn leave_kernel_scope(conn: &mut PgConnection) -> Result<(), sqlx::Error> {
    sqlx::query_scalar!("SELECT set_config($1, '', true)", KERNEL_SCOPE_SETTING)
        .fetch_one(conn)
        .await?;
    Ok(())
}

/// Begin a transaction working in `workspace`, given as text (the kernel's
/// tables keep workspaces as text): [`BeginFor::begin_for`] for the
/// kernel's own per-workspace work.
pub(crate) async fn begin_in(
    pool: &PgPool,
    workspace: &str,
) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    set_workspace(&mut tx, workspace).await?;
    Ok(tx)
}

/// What a transaction may reach on the kernel's tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Scope {
    /// The workspace it works in ([`WORKSPACE_SETTING`]).
    pub workspace: Option<String>,
    /// Whether the kernel scope is on ([`KERNEL_SCOPE_SETTING`]).
    pub kernel: bool,
}

impl Scope {
    /// Whether rows of `workspace` are within reach.
    pub fn reaches(&self, workspace: &str) -> bool {
        self.kernel || self.workspace.as_deref() == Some(workspace)
    }

    /// Neither a workspace nor the kernel scope: no row is visible.
    pub fn is_unset(&self) -> bool {
        !self.kernel && self.workspace.is_none()
    }
}

/// The scope of the current transaction.
pub(crate) async fn scope(conn: &mut PgConnection) -> Result<Scope, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT nullif(current_setting($1, true), '') AS workspace,
                  coalesce(current_setting($2, true), '') = 'on' AS "kernel!""#,
        WORKSPACE_SETTING,
        KERNEL_SCOPE_SETTING,
    )
    .fetch_one(conn)
    .await?;
    Ok(Scope {
        workspace: row.workspace,
        kernel: row.kernel,
    })
}

/// Refuse work on `workspace`'s kernel rows (`what`) in a transaction that
/// cannot reach them: one working in another workspace, or in none and
/// without the kernel scope (it would see and change nothing, silently).
pub(crate) async fn require_scope(
    conn: &mut PgConnection,
    workspace: &str,
    what: &str,
) -> Result<(), sqlx::Error> {
    let scope = scope(conn).await?;
    if scope.reaches(workspace) {
        return Ok(());
    }
    Err(sqlx::Error::InvalidArgument(if scope.is_unset() {
        format!(
            "{what} belongs to a workspace; run it in a transaction working there \
             (`begin_for`, a job's `tx()`), not one that set no workspace"
        )
    } else {
        format!("{what} belongs to another workspace than the transaction's")
    }))
}

/// Before the kernel writes an instance-wide row (no workspace: an
/// account's event, a scheduled tick) for a transaction that set nothing,
/// lift it into the kernel scope for that statement. Returns whether it
/// lifted; pass that to [`lower`] right after the statement.
///
/// **Invariant** (the callers, `EventBus::publish` and `jobs::insert`, keep
/// it):
/// - it lifts only a transaction that set **nothing** (no workspace, no
///   kernel scope); a workspace or kernel transaction is left as it is;
/// - between the lift and [`lower`] runs only the kernel's own statement,
///   whose workspace parameter is `None`;
/// - [`lower`] turns the scope off only if this lifted it, so a caller's
///   own kernel scope is never turned off;
/// - on an error between the two the transaction is aborted by Postgres
///   (or carries no change worth keeping), and the caller returns the
///   error without lowering;
/// - a publish or enqueue future dropped mid-statement (a cancelled task, a
///   lost `select!`) would leave the scope on: a caller must then drop the
///   transaction too, never keep using it. Every caller awaits these to
///   completion inside its own transaction's lifetime.
///
/// From a transaction working in a workspace (and not in the kernel scope)
/// it lifts nothing and adds only what a workspace may add
/// (`from_workspaces`: an instance-wide event declared
/// [`PUBLISHED_IN_WORKSPACES`](crate::events::DomainEvent::PUBLISHED_IN_WORKSPACES));
/// anything else (`what`) is refused here, as row-level security would
/// refuse it (campbooks-cloud#207).
pub(crate) async fn lift_for_instance_rows(
    conn: &mut PgConnection,
    what: &str,
    from_workspaces: bool,
) -> Result<bool, sqlx::Error> {
    let scope = scope(conn).await?;
    if scope.is_unset() {
        enter_kernel_scope(conn).await?;
        return Ok(true);
    }
    if !scope.kernel && !from_workspaces {
        return Err(sqlx::Error::InvalidArgument(format!(
            "{what} is instance-wide, and a transaction working in a workspace may not add it: \
             add it from a transaction that set no workspace, or declare an instance-wide \
             event `PUBLISHED_IN_WORKSPACES` (and list it in \
             `kernel.instance_rows_from_workspaces`)"
        )));
    }
    Ok(false)
}

/// Undo [`lift_for_instance_rows`]: only when it lifted.
pub(crate) async fn lower(conn: &mut PgConnection, lifted: bool) -> Result<(), sqlx::Error> {
    if lifted {
        leave_kernel_scope(conn).await?;
        #[cfg(debug_assertions)]
        {
            let after = scope(conn).await?;
            debug_assert!(after.is_unset(), "lowered to {after:?}, not unset");
        }
    }
    Ok(())
}

/// The schema migrations applied to the database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationStatus {
    /// How many migrations applied successfully.
    pub applied: i64,
    /// The newest one's version (`<timestamp>` of its file name).
    pub latest_version: Option<i64>,
    /// The newest one's description.
    pub latest_description: Option<String>,
}

/// What `sqlx`'s migration table says was applied.
pub async fn migration_status(pool: &PgPool) -> Result<MigrationStatus, sqlx::Error> {
    sqlx::query_as!(
        MigrationStatus,
        r#"SELECT count(*) AS "applied!",
                  max(version) AS latest_version,
                  (SELECT description FROM _sqlx_migrations
                    WHERE success ORDER BY version DESC LIMIT 1) AS latest_description
             FROM _sqlx_migrations WHERE success"#
    )
    .fetch_one(pool)
    .await
    .map(|status| MigrationStatus {
        // `admin__init.sql` is described as "admin  init".
        latest_description: status
            .latest_description
            .map(|d| d.split_whitespace().collect::<Vec<_>>().join(" ")),
        ..status
    })
}

/// `options` whose every new connection switches to `role` (`SET ROLE`)
/// before it is used. `None` leaves connections as the login user.
pub fn with_runtime_role(options: PgPoolOptions, role: Option<&str>) -> PgPoolOptions {
    let Some(role) = role.map(ToOwned::to_owned) else {
        return options;
    };
    options.after_connect(move |conn, _meta| {
        let role = role.clone();
        Box::pin(async move {
            // `set_config('role', …)` is `SET ROLE` with a bound parameter.
            sqlx::query_scalar!("SELECT set_config('role', $1, false)", role)
                .fetch_one(conn)
                .await?;
            Ok(())
        })
    })
}

/// What the database says about the role a pool's connections run as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleReport {
    /// `current_user`: the role queries run as (after any `SET ROLE`).
    pub user: String,
    /// `session_user`: the login the connection authenticated as.
    pub login: String,
    /// `user` is, or is a member of, a superuser or `BYPASSRLS` role, so
    /// row-level security would not apply to it.
    pub bypasses_row_security: bool,
    /// `user` owns (or is a member of the owner of) a table with row-level
    /// security: owners may disable or replace their own policies.
    pub owns_policy_tables: bool,
    /// `login` could become a superuser or `BYPASSRLS` role (it is one, or a
    /// member of one): a query that can run `RESET ROLE` or `SET ROLE` then
    /// escapes row-level security.
    pub login_can_bypass: bool,
    /// `user` is a member of `campbooks_definer`, the owner of the
    /// `SECURITY DEFINER` functions: it could reach the tables only those
    /// functions should (the web sessions' sealed tokens, the rate-limit
    /// counters) directly (campbooks-cloud#213, item 8).
    pub holds_definer_rights: bool,
}

/// Inspect the role `pool`'s connections run as.
pub async fn inspect_role(pool: &PgPool) -> Result<RoleReport, sqlx::Error> {
    sqlx::query_as!(
        RoleReport,
        r#"SELECT current_user::text AS "user!",
                  session_user::text AS "login!",
                  EXISTS (SELECT 1 FROM pg_roles r
                           WHERE (r.rolsuper OR r.rolbypassrls)
                             AND pg_has_role(current_user, r.oid, 'MEMBER')) AS "bypasses_row_security!",
                  EXISTS (SELECT 1 FROM pg_class c
                           WHERE c.relrowsecurity
                             AND pg_has_role(current_user, c.relowner, 'MEMBER')) AS "owns_policy_tables!",
                  EXISTS (SELECT 1 FROM pg_roles r
                           WHERE (r.rolsuper OR r.rolbypassrls)
                             AND pg_has_role(session_user, r.oid, 'MEMBER')) AS "login_can_bypass!",
                  EXISTS (SELECT 1 FROM pg_roles r
                           WHERE r.rolname = 'campbooks_definer'
                             AND pg_has_role(current_user, r.oid, 'MEMBER')) AS "holds_definer_rights!""#
    )
    .fetch_one(pool)
    .await
}

/// What a runtime role should do about its findings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleVerdict {
    /// Isolation holds with nothing a SQL-injection could undo.
    Sound,
    /// Isolation holds, but only through `SET ROLE` from a privileged login
    /// that a query could `RESET ROLE` back to. The legacy single-URL setup.
    Escapable(String),
    /// Row-level security does not protect this connection.
    Unsafe(String),
}

impl RoleReport {
    /// Judge the report. `split` is whether the operator configured a
    /// separate migration login: asking for the split and then running as a
    /// privileged login is [`RoleVerdict::Unsafe`], whereas a single
    /// privileged URL kept working through `SET ROLE` is only
    /// [`RoleVerdict::Escapable`].
    pub fn verdict(&self, split: bool) -> RoleVerdict {
        if self.bypasses_row_security {
            return RoleVerdict::Unsafe(format!(
                "the database role `{}` is a superuser or has BYPASSRLS (or is a member of \
                 such a role), so row-level security does not apply to it",
                self.user
            ));
        }
        if self.owns_policy_tables {
            return RoleVerdict::Unsafe(format!(
                "the database role `{}` owns tables that carry row-level security \
                 policies, and owners can change their own policies",
                self.user
            ));
        }
        if self.holds_definer_rights {
            return RoleVerdict::Unsafe(format!(
                "the database role `{}` is a member of `campbooks_definer`, the owner of the \
                 SECURITY DEFINER functions, so it can read the web sign-in's tables (sealed \
                 tokens) and the rate-limit counters directly; revoke that membership",
                self.user
            ));
        }
        if self.login_can_bypass {
            let why = format!(
                "the database login `{}` can bypass row-level security and the server only \
                 switches away from it with SET ROLE, which a SQL injection could undo",
                self.login
            );
            return if split {
                RoleVerdict::Unsafe(why)
            } else {
                RoleVerdict::Escapable(why)
            };
        }
        RoleVerdict::Sound
    }
}

/// Connection options for `url`, with the password read from `password_file`
/// when one is given (a trailing newline is dropped). The file keeps the
/// secret out of the environment and the process list.
///
/// # Errors
/// If the URL is malformed or the file cannot be read.
pub fn connect_options(
    url: &str,
    password_file: Option<&std::path::Path>,
) -> Result<PgConnectOptions, String> {
    let options = url
        .parse::<PgConnectOptions>()
        .map_err(|e| format!("the database URL is not valid: {e}"))?;
    let Some(path) = password_file else {
        return Ok(options);
    };
    let password = std::fs::read_to_string(path).map_err(|e| {
        format!(
            "could not read the database password from {}: {e}",
            path.display()
        )
    })?;
    Ok(options.password(password.trim_end_matches(['\r', '\n'])))
}

/// Open the runtime pool, its connections bounded by [`with_session_limits`].
/// The `role` switch (`SET ROLE`) is applied only when
/// the login can bypass row-level security: an unprivileged login already
/// inherits the runtime role's grants, and needs no way back to anything.
///
/// # Errors
/// If the database cannot be reached or refuses the login.
pub async fn connect_runtime(
    options: PgConnectOptions,
    max_connections: u32,
    acquire_timeout: std::time::Duration,
    role: Option<&str>,
) -> Result<PgPool, sqlx::Error> {
    let options = with_session_limits(options);
    let mut probe = PgConnection::connect_with(&options).await?;
    let privileged = login_can_bypass(&mut probe).await?;
    let _ = probe.close().await;
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(acquire_timeout)
        // One `sqlx::pool::acquire` event per acquire, for the server's
        // pool-wait metric and span events; log filters keep it out of logs.
        .acquire_time_level(log::LevelFilter::Debug);
    with_runtime_role(pool, role.filter(|_| privileged))
        .connect_with(options)
        .await
}

/// Whether the connection's login is, or is a member of, a superuser or
/// `BYPASSRLS` role.
pub async fn login_can_bypass(conn: &mut PgConnection) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM pg_roles r
                           WHERE (r.rolsuper OR r.rolbypassrls)
                             AND pg_has_role(session_user, r.oid, 'MEMBER')) AS "can!""#
    )
    .fetch_one(conn)
    .await
}

/// The oldest PostgreSQL Campbooks runs on, as `server_version_num`: 15,
/// for `NULLS NOT DISTINCT` (the job queue's unique keys). Campbooks is
/// built and tested on 17, the bundled image (campbooks-cloud#213, item 15).
pub const MIN_SERVER_VERSION_NUM: i32 = 150_000;

/// The connected server's `server_version_num` (e.g. `170002`).
pub async fn server_version_num(conn: &mut PgConnection) -> Result<i32, sqlx::Error> {
    sqlx::query_scalar("SELECT current_setting('server_version_num')::int")
        .fetch_one(conn)
        .await
}

/// Refuse a server older than [`MIN_SERVER_VERSION_NUM`], naming both.
///
/// # Errors
/// When `version_num` is older.
pub fn check_server_version(version_num: i32) -> Result<(), String> {
    if version_num >= MIN_SERVER_VERSION_NUM {
        return Ok(());
    }
    Err(format!(
        "PostgreSQL {}.{} is too old: Campbooks needs PostgreSQL {} or later (17 \
         recommended, the version it is built and tested on)",
        version_num / 10_000,
        version_num % 10_000,
        MIN_SERVER_VERSION_NUM / 10_000
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_15_or_later() {
        assert!(check_server_version(170_002).is_ok());
        assert!(check_server_version(150_000).is_ok());
        let old = check_server_version(140_011).unwrap_err();
        assert!(old.contains("PostgreSQL 14.11 is too old"), "{old}");
        assert!(old.contains("PostgreSQL 15 or later"), "{old}");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn the_test_server_is_new_enough(db: PgPool) {
        let mut conn = db.acquire().await.unwrap();
        let version = server_version_num(&mut conn).await.unwrap();
        assert!(check_server_version(version).is_ok(), "{version}");
    }

    fn report() -> RoleReport {
        RoleReport {
            user: "campbooks_runtime".into(),
            login: "campbooks_runtime".into(),
            bypasses_row_security: false,
            owns_policy_tables: false,
            login_can_bypass: false,
            holds_definer_rights: false,
        }
    }

    /// `name`'s value on a connection of `pool`.
    async fn setting(pool: &PgPool, name: &str) -> String {
        sqlx::query_scalar::<_, String>("SELECT current_setting($1)")
            .bind(name)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn runtime_connections_never_wait_for_ever(db: PgPool) {
        let runtime = connect_runtime(
            (*db.connect_options()).clone(),
            2,
            Duration::from_secs(5),
            None,
        )
        .await
        .unwrap();
        assert_eq!(setting(&runtime, "statement_timeout").await, "30s");
        assert_eq!(setting(&runtime, "lock_timeout").await, "10s");
        // The tests' runtime-role pool too.
        let app = crate::testing::app_role_pool(&db).await;
        assert_eq!(setting(&app, "statement_timeout").await, "30s");
        assert_eq!(setting(&app, "lock_timeout").await, "10s");
        // A job's transaction may run longer statements, until it ends.
        let mut tx = runtime.begin().await.unwrap();
        allow_statements_for(&mut tx, JOB_STATEMENT_TIMEOUT)
            .await
            .unwrap();
        let inside: String = sqlx::query_scalar("SELECT current_setting('statement_timeout')")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert_eq!(inside, "5min");
        tx.rollback().await.unwrap();
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn a_statement_or_lock_that_times_out_is_a_503(db: PgPool) {
        use axum::http::StatusCode;

        // A statement past its time.
        let mut tx = db.begin().await.unwrap();
        sqlx::query("SET LOCAL statement_timeout = '20ms'")
            .execute(&mut *tx)
            .await
            .unwrap();
        let slow = sqlx::query("SELECT pg_sleep(2)")
            .execute(&mut *tx)
            .await
            .unwrap_err();
        assert_eq!(
            crate::Error::from(slow).status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        tx.rollback().await.unwrap();
        // A lock held elsewhere, waited on past its time.
        let mut holder = db.begin().await.unwrap();
        sqlx::query("LOCK TABLE kernel.change_heads IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *holder)
            .await
            .unwrap();
        let mut waiter = db.begin().await.unwrap();
        sqlx::query("SET LOCAL lock_timeout = '20ms'")
            .execute(&mut *waiter)
            .await
            .unwrap();
        let locked = sqlx::query("SELECT 1 FROM kernel.change_heads LIMIT 1")
            .execute(&mut *waiter)
            .await
            .unwrap_err();
        let error = crate::Error::from(locked);
        assert_eq!(error.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(error.code(), "database_unavailable");
        waiter.rollback().await.unwrap();
        holder.rollback().await.unwrap();
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn an_admin_transaction_reads_across_workspaces_until_it_ends(db: PgPool) {
        use crate::admin::{AdminGrant, PermissionSet};

        let pool = crate::testing::app_role_pool(&db).await;
        let grant = AdminGrant::new("a-1".into(), PermissionSet::from_names(["super"]));
        let setting = "SELECT coalesce(current_setting('campbooks.admin_reads', true), '')";
        let mut tx = begin_admin(&pool, &grant).await.unwrap();
        let on: String = sqlx::query_scalar(setting)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert_eq!(on, "on");
        tx.commit().await.unwrap();
        let mut conn = pool.acquire().await.unwrap();
        let after: String = sqlx::query_scalar(setting)
            .fetch_one(&mut *conn)
            .await
            .unwrap();
        assert_eq!(after, "", "transaction-local");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn the_runtime_role_reads_the_migration_status(db: PgPool) {
        let pool = crate::testing::app_role_pool(&db).await;
        let status = migration_status(&pool).await.unwrap();
        assert!(status.applied > 10, "{status:?}");
        assert!(
            status
                .latest_version
                .is_some_and(|v| v > 20_261_012_000_000)
        );
        assert!(status.latest_description.is_some());
    }

    #[test]
    fn an_unprivileged_login_is_sound() {
        assert_eq!(report().verdict(false), RoleVerdict::Sound);
        assert_eq!(report().verdict(true), RoleVerdict::Sound);
    }

    #[test]
    fn bypassing_or_owning_is_unsafe_either_way() {
        for split in [false, true] {
            let bypass = RoleReport {
                bypasses_row_security: true,
                ..report()
            };
            assert!(matches!(bypass.verdict(split), RoleVerdict::Unsafe(_)));
            let owner = RoleReport {
                owns_policy_tables: true,
                ..report()
            };
            assert!(matches!(owner.verdict(split), RoleVerdict::Unsafe(_)));
            let definer = RoleReport {
                holds_definer_rights: true,
                ..report()
            };
            assert!(matches!(definer.verdict(split), RoleVerdict::Unsafe(_)));
        }
    }

    #[test]
    fn a_privileged_login_is_only_tolerated_without_the_split() {
        let switched = RoleReport {
            login_can_bypass: true,
            ..report()
        };
        assert!(matches!(switched.verdict(false), RoleVerdict::Escapable(_)));
        assert!(matches!(switched.verdict(true), RoleVerdict::Unsafe(_)));
    }

    #[test]
    fn the_password_comes_from_the_file() {
        let dir = std::env::temp_dir().join(format!("cb-pw-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("password");
        std::fs::write(&file, "s3cret\n").unwrap();
        let options = connect_options(
            "postgres://campbooks_runtime@db.example.com/campbooks",
            Some(&file),
        )
        .unwrap();
        assert_eq!(options.get_username(), "campbooks_runtime");
        assert!(format!("{options:?}").contains("s3cret"));
        assert!(connect_options("postgres://u@h/d", Some(&dir.join("missing"))).is_err());
        assert!(connect_options("not a url", None).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
