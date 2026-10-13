//! The second setting this module's row-level security policies read: the
//! signed-in user, for the few reads that span workspaces (my workspaces, my
//! memberships, my own instance-level security events).

use campbooks_kernel::tenancy::UserId;
use sqlx::{PgConnection, PgPool, Postgres, Transaction};

/// The transaction-local setting carrying the signed-in user.
pub const USER_SETTING: &str = "campbooks.user_id";

/// Set [`USER_SETTING`] until the current transaction ends.
pub async fn set_user(conn: &mut PgConnection, user: UserId) -> Result<(), sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT set_config($1, $2, true)",
        USER_SETTING,
        user.to_string()
    )
    .fetch_one(conn)
    .await?;
    Ok(())
}

/// Begin a transaction that works as `user`, in no workspace.
pub async fn begin_as_user(
    db: &PgPool,
    user: UserId,
) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
    let mut tx = db.begin().await?;
    set_user(&mut tx, user).await?;
    Ok(tx)
}
