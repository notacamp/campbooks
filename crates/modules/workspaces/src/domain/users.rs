//! Users, created on their first sign-in from their access token.

use campbooks_kernel::{
    Error, Result,
    auth::{Principal, PrincipalKind},
    tenancy::UserId,
};
use sqlx::PgPool;

use super::{
    db::begin_as_user,
    security::{self, SecurityKind},
};

/// The user behind `principal`, created on first sight (recording a
/// `signed_in` security event) and kept in step with the name and email
/// their token carries. Client tokens are no user: 403.
pub async fn ensure_user(db: &PgPool, principal: &Principal) -> Result<UserId> {
    if principal.kind != PrincipalKind::User {
        return Err(Error::Forbidden);
    }
    if let Some(id) = refresh(db, principal).await? {
        return Ok(id);
    }

    let id = UserId::new();
    let mut tx = begin_as_user(db, id).await?;
    let inserted = sqlx::query_scalar!(
        r#"
        INSERT INTO workspaces.users (id, idp_subject, display_name, email)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (idp_subject) DO NOTHING
        RETURNING id AS "id: UserId"
        "#,
        id as UserId,
        principal.subject,
        principal.name,
        principal.email,
    )
    .fetch_optional(&mut *tx)
    .await?;
    if inserted.is_none() {
        // Created by a concurrent first request of the same user.
        tx.rollback().await?;
        return refresh(db, principal).await?.ok_or(Error::NotFound);
    }
    security::record(
        &mut tx,
        None,
        Some(id),
        SecurityKind::SignedIn,
        serde_json::json!({ "first": true, "client_id": principal.client_id }),
    )
    .await?;
    tx.commit().await?;
    tracing::info!(user = %id, "first sign-in");
    Ok(id)
}

/// The existing user of `principal`, with a changed name or email updated.
async fn refresh(db: &PgPool, principal: &Principal) -> Result<Option<UserId>> {
    let Some(row) = sqlx::query!(
        r#"SELECT id AS "id: UserId", display_name, email
             FROM workspaces.users WHERE idp_subject = $1"#,
        principal.subject
    )
    .fetch_optional(db)
    .await?
    else {
        return Ok(None);
    };
    let name_changed = principal.name.is_some() && principal.name != row.display_name;
    let email_changed = principal.email.is_some() && principal.email != row.email;
    if name_changed || email_changed {
        sqlx::query!(
            "UPDATE workspaces.users
                SET display_name = coalesce($2, display_name), email = coalesce($3, email)
              WHERE id = $1",
            row.id as UserId,
            principal.name,
            principal.email,
        )
        .execute(db)
        .await?;
    }
    Ok(Some(row.id))
}
