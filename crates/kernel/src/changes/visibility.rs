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

/// Several modules' [`Visibility`] ports as one: a container is answered by
/// the port that declares its type in
/// [`container_types`](Visibility::container_types); a container of a type
/// none declares is shown to nobody.
#[derive(Debug, Clone)]
pub struct Visibilities {
    ports: Vec<VisibilityPort>,
    access_records: &'static [&'static str],
    container_types: &'static [&'static str],
}

impl Visibilities {
    /// The ports as one. Built once, at start: the record and container
    /// types they declare are kept for the life of the process.
    pub fn new(ports: Vec<VisibilityPort>) -> Self {
        let mut access_records: Vec<&'static str> = Vec::new();
        let mut container_types: Vec<&'static str> = Vec::new();
        for port in &ports {
            access_records.extend(port.access_records());
            container_types.extend(port.container_types());
        }
        Self {
            ports,
            access_records: Box::leak(access_records.into_boxed_slice()),
            container_types: Box::leak(container_types.into_boxed_slice()),
        }
    }

    fn answering(&self, container_type: &str) -> Option<&VisibilityPort> {
        self.ports
            .iter()
            .find(|port| port.container_types().contains(&container_type))
    }
}

impl Visibility for Visibilities {
    fn container_visible<'a>(
        &'a self,
        db: &'a PgPool,
        access: &'a Access,
        container: &'a RecordRef,
    ) -> VisibilityFuture<'a> {
        match self.answering(&container.record_type) {
            Some(port) => port.container_visible(db, access, container),
            None => Box::pin(async { Ok(false) }),
        }
    }

    fn access_records(&self) -> &'static [&'static str] {
        self.access_records
    }

    fn container_types(&self) -> &'static [&'static str] {
        self.container_types
    }

    fn visible_containers<'a>(
        &'a self,
        db: &'a PgPool,
        access: &'a Access,
        container_type: &'a str,
    ) -> ContainersFuture<'a> {
        match self.answering(container_type) {
            Some(port) => port.visible_containers(db, access, container_type),
            None => Box::pin(async { Ok(Vec::new()) }),
        }
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
