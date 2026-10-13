//! The container-visibility port: which members may see a change that lives
//! inside a container (an inbox, a shared calendar, a folder).

use std::{future::Future, pin::Pin, sync::Arc};

use sqlx::PgPool;

use crate::{Error, records::RecordRef, tenancy::Access};

/// A future returned by a [`Visibility`] port.
pub type VisibilityFuture<'a> = Pin<Box<dyn Future<Output = Result<bool, Error>> + Send + 'a>>;

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
