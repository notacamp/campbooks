//! `campbooks-admin` — who administers this instance, and what they did
//! (ADR 0015).
//!
//! The module owns schema `admin`: **profiles** (an identity account id and
//! its [`Permission`](campbooks_kernel::admin::Permission)s) and the
//! **actions** audit trail. It serves **no routes**, in either API: the rest
//! of the system reads profiles and appends to the trail only through the
//! kernel ports it implements ([`profiles`], [`audit`]), and profiles are
//! created, changed, listed and removed only by the server's CLI through
//! [`cli`], on the migration login. The runtime database role may read
//! profiles and append actions, nothing else. `cargo xtask arch` fails if
//! this module gains a route or anything else names its schema.

#![forbid(unsafe_code)]

use std::sync::Arc;

use campbooks_kernel::{
    Background, Error, Module,
    admin::{
        AdminAction, AdminAudit, AdminAuditPort, AdminProfiles, AdminProfilesPort, AuditFuture,
        PermissionSet, ProfilesFuture,
    },
};
use sqlx::PgPool;
use utoipa_axum::router::OpenApiRouter;
use uuid::Uuid;

pub mod cli;
pub mod scopes;

/// Module name: crate directory and Postgres schema.
pub const NAME: &str = "admin";

/// The module's part of the access catalogue (ADR 0017): admin profiles and
/// the audit trail are reached by no operation; only the server's CLI
/// writes them.
static RESOURCES: [campbooks_kernel::authz::ResourceType; 2] = {
    use campbooks_kernel::authz::{ResourceType, Tenancy};
    [
        ResourceType::new(
            "admin.profile",
            "An administrator's profile (their permissions): written by `campbooks-server \
             admin` only, read by the kernel's admin check.",
            Tenancy::Instance,
        ),
        ResourceType::new(
            "admin.action",
            "An entry of the admin audit trail: appended by the admin API, read with \
             `campbooks-server admin log`.",
            Tenancy::Instance,
        ),
    ]
};

/// The admin module: a scope, no routes, no background work.
pub fn module() -> Module {
    Module {
        name: NAME,
        scopes: scopes::ALL,
        router: OpenApiRouter::new(),
        admin_router: None,
        background: Background::new(),
        resources: campbooks_kernel::authz::Resources::declared(&RESOURCES),
    }
}

/// The kernel's admin-profile port, reading `admin.profiles`.
pub fn profiles() -> AdminProfilesPort {
    Arc::new(Profiles)
}

/// The kernel's admin audit port, appending to `admin.actions`.
pub fn audit() -> AdminAuditPort {
    Arc::new(Audit)
}

#[derive(Debug, Clone, Copy)]
struct Profiles;

impl AdminProfiles for Profiles {
    fn permissions_of<'a>(&'a self, db: &'a PgPool, account: &'a str) -> ProfilesFuture<'a> {
        Box::pin(async move {
            let names: Option<Vec<String>> = sqlx::query_scalar!(
                "SELECT permissions FROM admin.profiles WHERE account_id = $1",
                account
            )
            .fetch_optional(db)
            .await?;
            Ok(PermissionSet::from_names(names.unwrap_or_default()))
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct Audit;

impl AdminAudit for Audit {
    fn record<'a>(&'a self, db: &'a PgPool, action: &'a AdminAction) -> AuditFuture<'a> {
        Box::pin(async move {
            sqlx::query!(
                "INSERT INTO admin.actions (id, at, actor, via, operation, target, status)
                 VALUES ($1, $2, $3, 'api', $4, $5, $6)",
                Uuid::now_v7(),
                action.at,
                action.actor,
                action.operation,
                action.target,
                i16::try_from(action.status).unwrap_or(i16::MAX),
            )
            .execute(db)
            .await
            .map_err(Error::from)?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests;
