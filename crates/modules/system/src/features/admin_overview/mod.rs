//! `GET /admin/v1/system/overview` — the System panel of the admin
//! dashboard (ADR 0015): which build runs, since when, whether the database
//! answers and which migrations it has, the effective configuration without
//! a single secret, and whether each AI role's provider answers.
//!
//! An admin operation: served by the admin API only, never in the operation
//! catalogue. Requires `system.read` (or `super`).

use std::time::{Duration, Instant};

use axum::{Extension, Json, extract::State};
use campbooks_kernel::{
    AppState, Result,
    admin::{Admin, AdminOperation, permission::SystemRead},
    config::ConfigSummary,
    db,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{Deps, deps::Facts, features::version::VersionResponse};

/// How long the database gets to answer the overview's probe.
const DATABASE_TIMEOUT: Duration = Duration::from_secs(3);

/// The instance at a glance, for administrators.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AdminOverview {
    /// Semantic version of the server.
    #[schema(example = "0.1.0")]
    pub version: String,
    /// Git commit the build came from, when known.
    pub commit: Option<String>,
    /// When this server process started.
    pub started_at: DateTime<Utc>,
    /// Seconds since then.
    pub uptime_seconds: i64,
    /// Whether the database answers, and its migrations.
    pub database: DatabaseStatus,
    /// The effective configuration: enumerated facts only, no secret.
    pub configuration: ConfigSummary,
    /// Who signs people in, and which providers are set up.
    pub services: Facts,
    /// Each AI role: configured or not, model, provider and whether it
    /// answers.
    pub ai: Vec<AiRoleStatus>,
}

/// The database, as this server sees it.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DatabaseStatus {
    /// Whether it answered within 3 seconds.
    pub reachable: bool,
    /// How long a trivial query took, in milliseconds.
    pub latency_ms: Option<u64>,
    /// How many schema migrations are applied.
    pub migrations_applied: Option<i64>,
    /// The newest applied migration's version (`YYYYMMDDHHMMSS`).
    pub latest_migration: Option<i64>,
    /// The newest applied migration's description.
    pub latest_migration_description: Option<String>,
}

/// An AI role (ADR 0005).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AiRole {
    /// Typed decisions.
    Decide,
    /// Embeddings.
    Embed,
    /// Free-form generation.
    Complete,
    /// Images and scans.
    Vision,
}

/// The protocol a provider speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AiProtocol {
    /// The OpenAI-compatible API.
    Openai,
    /// Laya's typed decision protocol.
    Laya,
}

/// Where an AI role's endpoint is, as far as mail content is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AiLocation {
    /// On the instance's own network (loopback or private address, a
    /// compose service name, `localhost`, or `CAMPBOOKS_AI__INTERNAL_HOSTS`).
    Instance,
    /// Anywhere else: gets mail content only when the role opts in.
    External,
}

/// One AI role, without a secret: the provider is shown as scheme, host
/// and port only, and keys never.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AiRoleStatus {
    /// The role.
    pub role: AiRole,
    /// Whether a provider is configured for it.
    pub configured: bool,
    /// The model it calls.
    pub model: Option<String>,
    /// The provider's scheme, host and port.
    #[schema(example = "http://ollama:11434")]
    pub endpoint: Option<String>,
    /// The protocol it speaks.
    pub protocol: Option<AiProtocol>,
    /// Whether its endpoint is on the instance's network.
    pub location: Option<AiLocation>,
    /// Whether it may receive mail content: on the instance's network, or
    /// opted in with `CAMPBOOKS_AI__<ROLE>__EXTERNAL_CONTENT=allow`. When
    /// `false`, nothing calls it with content (the decide pass leaves
    /// sorting to rules); only the reachability probe runs.
    pub content_allowed: Option<bool>,
    /// Whether the provider answered a probe within 3 seconds (probes are
    /// reused for a minute); `null` when not configured.
    pub reachable: Option<bool>,
}

impl From<campbooks_ai::RoleHealth> for AiRoleStatus {
    fn from(health: campbooks_ai::RoleHealth) -> Self {
        Self {
            role: match health.role {
                campbooks_ai::Role::Decide => AiRole::Decide,
                campbooks_ai::Role::Embed => AiRole::Embed,
                campbooks_ai::Role::Complete => AiRole::Complete,
                campbooks_ai::Role::Vision => AiRole::Vision,
            },
            configured: health.configured,
            model: health.model,
            endpoint: health.endpoint,
            protocol: health.protocol.map(|protocol| match protocol {
                campbooks_ai::Protocol::OpenAi => AiProtocol::Openai,
                campbooks_ai::Protocol::Laya => AiProtocol::Laya,
            }),
            location: health.location.map(|location| match location {
                campbooks_ai::Location::Instance => AiLocation::Instance,
                campbooks_ai::Location::External => AiLocation::External,
            }),
            content_allowed: health.content_allowed,
            reachable: health.reachable,
        }
    }
}

/// See the instance's version, health and configuration.
#[utoipa::path(
    get,
    path = "/admin/v1/system/overview",
    operation_id = "system.admin_overview",
    tag = "system",
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::query::<SystemRead>()))),
    responses(
        (status = OK, description = "The overview.", body = AdminOverview),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again (the last \
            sign-in is more than 12 hours old).", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not an administrator with `system.read` (the same \
            answer as an unknown route).", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    _admin: Admin<SystemRead>,
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
) -> Result<Json<AdminOverview>> {
    let build = VersionResponse::current();
    let now = Utc::now();
    let ai = match &deps.ai {
        Some(gateway) => gateway.health().await,
        None => campbooks_ai::unconfigured(),
    };
    Ok(Json(AdminOverview {
        version: build.version,
        commit: build.commit,
        started_at: deps.started_at,
        uptime_seconds: (now - deps.started_at).num_seconds().max(0),
        database: database(&state).await,
        configuration: state.config.summary(),
        services: deps.facts.clone(),
        ai: ai.into_iter().map(AiRoleStatus::from).collect(),
    }))
}

async fn database(state: &AppState) -> DatabaseStatus {
    let unreachable = DatabaseStatus {
        reachable: false,
        latency_ms: None,
        migrations_applied: None,
        latest_migration: None,
        latest_migration_description: None,
    };
    let started = Instant::now();
    let probe = tokio::time::timeout(DATABASE_TIMEOUT, async {
        sqlx::query_scalar!(r#"SELECT 1 AS "one!""#)
            .fetch_one(&state.db)
            .await?;
        let latency = started.elapsed();
        db::migration_status(&state.db)
            .await
            .map(|status| (latency, status))
    })
    .await;
    match probe {
        Ok(Ok((latency, status))) => DatabaseStatus {
            reachable: true,
            latency_ms: Some(u64::try_from(latency.as_millis()).unwrap_or(u64::MAX)),
            migrations_applied: Some(status.applied),
            latest_migration: status.latest_version,
            latest_migration_description: status.latest_description,
        },
        Ok(Err(error)) => {
            tracing::warn!(%error, "admin overview: database unreachable");
            unreachable
        }
        Err(_) => {
            tracing::warn!("admin overview: database timed out");
            unreachable
        }
    }
}

#[cfg(test)]
mod tests;
