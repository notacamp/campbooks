//! Whether each role's provider answers: what an instance administrator sees
//! in the system overview (ADR 0015). Nothing secret leaves here: base URLs
//! are reduced to scheme, host and port, and keys are never shown.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use secrecy::ExposeSecret as _;
use serde::Serialize;

use crate::{Gateway, Protocol, ProviderConfig, Role, gateway::endpoint};

/// How long a probe may take before the provider counts as unreachable.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// How long a probe's answer is reused.
pub const PROBE_CACHE: Duration = Duration::from_mins(1);

/// One role, as an administrator sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoleHealth {
    /// The role.
    pub role: Role,
    /// Whether a provider is configured for it.
    pub configured: bool,
    /// The model it calls, when configured.
    pub model: Option<String>,
    /// The provider's scheme, host and port (never credentials or a path).
    pub endpoint: Option<String>,
    /// The protocol it speaks, when configured.
    pub protocol: Option<Protocol>,
    /// Whether the provider answered a probe (`GET <base_url>/models`, any
    /// non-5xx status) within [`PROBE_TIMEOUT`]; `None` when not configured.
    pub reachable: Option<bool>,
}

/// Every role, none configured.
pub fn unconfigured() -> Vec<RoleHealth> {
    Role::ALL
        .into_iter()
        .map(|role| RoleHealth {
            role,
            configured: false,
            model: None,
            endpoint: None,
            protocol: None,
            reachable: None,
        })
        .collect()
}

/// Recent probe answers per role. Shared by a gateway's clones.
#[derive(Debug, Clone, Default)]
pub(crate) struct Probes(Arc<Mutex<HashMap<Role, (Instant, bool)>>>);

impl Probes {
    fn get(&self, role: Role, now: Instant) -> Option<bool> {
        let probes = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        probes
            .get(&role)
            .filter(|(at, _)| now.duration_since(*at) < PROBE_CACHE)
            .map(|(_, reachable)| *reachable)
    }

    fn put(&self, role: Role, at: Instant, reachable: bool) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(role, (at, reachable));
    }
}

impl Gateway {
    /// Every role: configured or not, its model and endpoint, and whether
    /// its provider answers (probed at most once per [`PROBE_CACHE`], all
    /// roles at once).
    pub async fn health(&self) -> Vec<RoleHealth> {
        let mut probes = tokio::task::JoinSet::new();
        for role in Role::ALL {
            if let Ok(provider) = self.config().provider(role) {
                let gateway = self.clone();
                let provider = provider.clone();
                probes.spawn(async move { (role, gateway.reachable(role, &provider).await) });
            }
        }
        let mut reachable = HashMap::new();
        while let Some(Ok((role, answered))) = probes.join_next().await {
            reachable.insert(role, answered);
        }
        Role::ALL
            .into_iter()
            .map(|role| match self.config().provider(role) {
                Ok(provider) => RoleHealth {
                    role,
                    configured: true,
                    model: Some(provider.model.clone()),
                    endpoint: campbooks_kernel::config::origin_of(provider.base_url.as_str()),
                    protocol: Some(provider.protocol),
                    reachable: Some(reachable.get(&role).copied().unwrap_or(false)),
                },
                Err(_) => RoleHealth {
                    role,
                    configured: false,
                    model: None,
                    endpoint: None,
                    protocol: None,
                    reachable: None,
                },
            })
            .collect()
    }

    async fn reachable(&self, role: Role, provider: &ProviderConfig) -> bool {
        if let Some(cached) = self.probes.get(role, Instant::now()) {
            return cached;
        }
        let answered = self.probe(provider).await;
        self.probes.put(role, Instant::now(), answered);
        answered
    }

    async fn probe(&self, provider: &ProviderConfig) -> bool {
        let Ok(url) = endpoint(&provider.base_url, "models") else {
            return false;
        };
        let mut request = self.http().get(url).timeout(PROBE_TIMEOUT);
        if let Some(key) = &provider.api_key {
            request = request.bearer_auth(key.expose_secret());
        }
        match request.send().await {
            Ok(response) => !response.status().is_server_error(),
            Err(err) => {
                tracing::debug!(
                    timeout = err.is_timeout(),
                    connect = err.is_connect(),
                    "ai provider probe failed"
                );
                false
            }
        }
    }
}
