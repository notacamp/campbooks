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

    /// Record types whose changes may change who sees inside a container
    /// (a membership of an inbox ending, an inbox made for someone). A
    /// change stream keeps the containers it listed until one of these is
    /// numbered (whether or not its caller may see it) or its next
    /// heartbeat, so every way of gaining or losing sight inside a
    /// container must be announced by such a change. A gain that is not
    /// has its changes until the next heartbeat skipped: that heartbeat
    /// finds the container and sends `reset` (`access_changed`) so the
    /// client refetches.
    fn access_records(&self) -> &'static [&'static str] {
        &[]
    }

    /// The container types this port answers for. A change stream lists the
    /// caller's containers of each ([`visible_containers`](Self::visible_containers))
    /// and asks the database only for changes in those (or in none), so a
    /// change in a container of any other type reaches nobody.
    fn container_types(&self) -> &'static [&'static str] {
        &[]
    }

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
