//! Database conventions every module relies on (ADR 0011): the role queries
//! run as, and the workspace a transaction works in.
//!
//! **Row-level security.** Workspace-owned tables carry Postgres policies
//! that admit only the rows of the workspace in the transaction-local setting
//! [`WORKSPACE_SETTING`]. Superusers and `BYPASSRLS` roles skip policies, and
//! the database user a self-hoster connects with is often a superuser, so the
//! server's pool switches every connection to a **runtime role**
//! ([`DEFAULT_RUNTIME_ROLE`], `CAMPBOOKS_DATABASE_ROLE`) that is subject to
//! them. Migrations run as the login user on their own connection; the
//! kernel migration creates the runtime role, and each migration grants it
//! what its own tables need.
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

use sqlx::{PgConnection, PgPool, Postgres, Transaction, postgres::PgPoolOptions};

use crate::tenancy::WorkspaceId;

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

/// Whether `pool`'s connections run as a role that skips row-level security
/// (a superuser or `BYPASSRLS`), so the policies would protect nothing.
pub async fn bypasses_row_security(pool: &PgPool) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT (rolsuper OR rolbypassrls) AS "bypasses!" FROM pg_roles WHERE rolname = current_user"#
    )
    .fetch_one(pool)
    .await
}
