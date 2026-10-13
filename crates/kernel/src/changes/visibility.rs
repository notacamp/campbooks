//! The container-visibility port: which members may see a change that lives
//! inside a container (an inbox, a shared calendar, a folder).

use std::{future::Future, pin::Pin, sync::Arc};

use sqlx::PgPool;

use crate::{Error, records::RecordRef, tenancy::Access};

/// A future returned by a [`Visibility`] port.
pub type VisibilityFuture<'a> = Pin<Box<dyn Future<Output = Result<bool, Error>> + Send + 'a>>;

/// A future returned by [`Visibility::visible_containers`].
pub type ContainersFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<RecordRef>, Error>> + Send + 'a>>;

/// Answers "may this member see what lives in this container?" for the
/// change feed. Modules that introduce containers implement it and the
/// server registers it with
/// [`AppState::with_visibility`](crate::AppState::with_visibility). A stream
/// asks once per container and connection.
pub trait Visibility: std::fmt::Debug + Send + Sync + 'static {
    /// Whether the caller with `access` may see records inside `container`.
    fn container_visible<'a>(
        &'a self,
        db: &'a PgPool,
        access: &'a Access,
        container: &'a RecordRef,
    ) -> VisibilityFuture<'a>;

    /// Every container of `container_type` the caller with `access` may see
    /// records inside, for queries that span them all (search). None by
    /// default: a port that cannot list fails closed.
    fn visible_containers<'a>(
        &'a self,
        _db: &'a PgPool,
        _access: &'a Access,
        _container_type: &'a str,
    ) -> ContainersFuture<'a> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

/// The port before any module provides one: changes inside a container are
/// shown to nobody (fail closed); changes without one are shown to every
/// member and never reach the port.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoContainers;

impl Visibility for NoContainers {
    fn container_visible<'a>(
        &'a self,
        _db: &'a PgPool,
        _access: &'a Access,
        _container: &'a RecordRef,
    ) -> VisibilityFuture<'a> {
        Box::pin(async { Ok(false) })
    }
}

/// Shared handle to the registered [`Visibility`] port.
pub type VisibilityPort = Arc<dyn Visibility>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tenancy::WorkspaceId;

    #[tokio::test]
    async fn no_containers_lists_none() {
        let db = crate::testing::unreachable_pool();
        let access = Access {
            workspace: WorkspaceId::new(),
            user: None,
            role: None,
        };
        let listed = NoContainers
            .visible_containers(&db, &access, "conversations.inbox")
            .await
            .unwrap();
        assert!(listed.is_empty());
    }
}
