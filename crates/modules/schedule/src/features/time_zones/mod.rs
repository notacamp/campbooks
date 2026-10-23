//! The kernel's `TimeZones` port: a member's time zone, for the module that
//! reads an email's dates (a relative "Thursday at 10" means 10:00 where its
//! reader lives).

use std::sync::Arc;

use campbooks_kernel::{
    schedule::{TimeZoneFuture, TimeZones, TimeZonesPort},
    tenancy::{UserId, WorkspaceId},
};
use sqlx::PgConnection;

use crate::domain::settings;

#[derive(Debug, Clone, Copy)]
struct Settings;

/// The port, for the server to register.
pub fn time_zones() -> TimeZonesPort {
    Arc::new(Settings)
}

impl TimeZones for Settings {
    fn time_zone<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        _workspace: WorkspaceId,
        member: UserId,
    ) -> TimeZoneFuture<'a> {
        Box::pin(async move { Ok(settings::load(conn, member).await?.time_zone) })
    }
}
