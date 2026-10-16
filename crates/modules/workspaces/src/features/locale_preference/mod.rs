//! The kernel's `LocalePreference` port: which language did this user
//! choose? Called when a request's locale is first needed (a handler that
//! renders text, or an error response), at most once per request.

use std::sync::Arc;

use campbooks_kernel::{
    Error,
    auth::Principal,
    i18n::{Locale, LocaleFuture, LocalePreference, LocalePreferencePort},
};
use sqlx::PgPool;

/// The port implementation the server registers in `AppState`.
pub fn locale_preference() -> LocalePreferencePort {
    Arc::new(SavedLocale)
}

/// The `locale` of the user's row; `NULL` (not chosen) or an unknown user
/// answers `None`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SavedLocale;

impl LocalePreference for SavedLocale {
    fn preferred<'a>(&'a self, db: &'a PgPool, principal: &'a Principal) -> LocaleFuture<'a> {
        Box::pin(async move {
            // `users` is not workspace-owned: no row-level security to set.
            let saved = sqlx::query_scalar!(
                "SELECT locale FROM workspaces.users WHERE idp_subject = $1",
                principal.subject
            )
            .fetch_optional(db)
            .await?
            .flatten();
            saved
                .map(|tag| tag.parse::<Locale>().map_err(Error::internal))
                .transpose()
        })
    }
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::{auth::PrincipalKind, testing};
    use sqlx::PgPool;

    use super::*;
    use crate::domain::users::ensure_user;

    fn user(subject: &str) -> Principal {
        Principal {
            subject: subject.to_owned(),
            kind: PrincipalKind::User,
            client_id: Some("campbooks-web".to_owned()),
            scopes: std::collections::BTreeSet::new(),
            workspace: None,
            email: None,
            email_verified: false,
            name: None,
            expires_at: None,
            issued_at: None,
            auth_time: None,
        }
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn answers_the_saved_locale_only(db: PgPool) {
        let app_db = testing::app_role_pool(&db).await;
        let port = locale_preference();
        let ana = user("ana");
        ensure_user(&app_db, &ana).await.unwrap();

        // New users have chosen nothing; unknown users neither.
        assert_eq!(port.preferred(&app_db, &ana).await.unwrap(), None);
        assert_eq!(
            port.preferred(&app_db, &user("nobody")).await.unwrap(),
            None
        );

        sqlx::query("UPDATE workspaces.users SET locale = 'pt-PT' WHERE idp_subject = 'ana'")
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(
            port.preferred(&app_db, &ana).await.unwrap(),
            Some(Locale::PtPt)
        );
    }
}
