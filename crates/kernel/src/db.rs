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
//! sets the same setting.
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

use std::future::Future;

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
                             AND pg_has_role(session_user, r.oid, 'MEMBER')) AS "login_can_bypass!""#
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

/// Open the runtime pool. The `role` switch (`SET ROLE`) is applied only when
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

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> RoleReport {
        RoleReport {
            user: "campbooks_runtime".into(),
            login: "campbooks_runtime".into(),
            bypasses_row_security: false,
            owns_policy_tables: false,
            login_can_bypass: false,
        }
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
