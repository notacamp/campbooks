//! IMAP push with IDLE (campbooks-cloud#318): a pool of IDLE sessions on
//! mailboxes' Inboxes, in each process that runs sync jobs.
//!
//! The pool never reads mail. A session signs in like a sync run, opens
//! the Inbox read-only and waits in IDLE (RFC 2177); when the server says
//! the Inbox changed, it asks the stream's schedule for a run now
//! (`schedule::run_now`, collapsing into a pending one), at most once per
//! debounce interval. Every minute the pool renews the leases it holds
//! (`lease`), stops the sessions whose lease it lost, and claims free ones
//! while it has room. A session holds no database connection while it
//! waits.
//!
//! Which connections may IDLE is the sync runs' decision: a run offers its
//! connection's lease row when the server offers IDLE and the mailbox
//! changed lately, and withdraws it otherwise (`lease::offer`).

mod hold;
pub mod lease;
#[cfg(test)]
mod tests;

use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicU64},
    time::Duration,
};

use campbooks_kernel::AppState;
use tokio::{
    sync::watch,
    task::JoinSet,
    time::{Instant, MissedTickBehavior},
};
use uuid::Uuid;

use crate::{deps::Deps, domain::model::ConnectionId};

/// How often the pool renews its leases and claims more.
const RENEW_EVERY: Duration = Duration::from_mins(1);

/// How long a session may take to say `DONE` and `LOGOUT` at shutdown.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// The pool's clocks: from the settings, shorter in tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Timing {
    /// How long IDLE lasts before `DONE`, `NOOP` and `IDLE` again.
    pub refresh: Duration,
    /// At most one run asked for per this long, per stream.
    pub debounce: Duration,
    /// How often leases are renewed and claimed.
    pub renew: Duration,
    /// How long a lease lasts (the database's [`lease::LEASE_SECONDS`]).
    pub lease: Duration,
    /// The first wait after a failure, doubled per failure.
    pub backoff: Duration,
    /// The longest wait after a failure.
    pub backoff_max: Duration,
    /// Failures in a row before the lease is given up.
    pub give_up_after: u32,
    /// How long a given-up lease waits before any process claims it again.
    pub cooldown: Duration,
}

impl Timing {
    /// The clocks `deps` sets.
    fn of(deps: &Deps) -> Self {
        let imap = &deps.imap.config;
        Self {
            refresh: Duration::from_mins(u64::from(imap.idle_refresh_minutes)),
            debounce: Duration::from_secs(u64::from(imap.idle_debounce_seconds)),
            renew: RENEW_EVERY,
            lease: Duration::from_secs(lease::LEASE_SECONDS.unsigned_abs().into()),
            backoff: Duration::from_secs(5),
            backoff_max: Duration::from_mins(15),
            give_up_after: 6,
            cooldown: Duration::from_mins(30),
        }
    }

    /// How long without a renewal before every session stops: a minute
    /// before the leases can expire for another process.
    fn renew_deadline(&self) -> Duration {
        self.lease.saturating_sub(self.renew)
    }
}

/// What every session of a pool shares.
#[derive(Debug)]
pub(crate) struct Shared {
    deps: Deps,
    state: AppState,
    /// This process's id as a lease holder (new at every start).
    holder: Uuid,
    timing: Timing,
    /// Runs asked for (tests and logs).
    asked: AtomicU64,
}

#[cfg(test)]
impl Shared {
    /// Runs the pool's sessions asked for so far.
    pub(crate) fn asked(&self) -> u64 {
        self.asked.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// This process's id as a lease holder.
    pub(crate) fn holder(&self) -> Uuid {
        self.holder
    }
}

/// The IDLE sessions of one process.
#[derive(Debug, Clone)]
pub struct IdlePool {
    shared: Arc<Shared>,
}

/// The sessions a pool holds now.
#[derive(Debug)]
pub(crate) struct Held {
    /// Each held connection's stop signal.
    stops: HashMap<ConnectionId, watch::Sender<bool>>,
    /// The sessions' tasks.
    tasks: JoinSet<ConnectionId>,
    /// When the leases were last renewed.
    renewed: Instant,
}

impl Held {
    fn new() -> Self {
        Self {
            stops: HashMap::new(),
            tasks: JoinSet::new(),
            renewed: Instant::now(),
        }
    }

    /// The connections held now.
    pub(crate) fn connections(&self) -> Vec<ConnectionId> {
        self.stops.keys().copied().collect()
    }

    /// Stop `connection`'s session.
    fn stop(&mut self, connection: ConnectionId) {
        if let Some(stop) = self.stops.remove(&connection) {
            stop.send_replace(true);
        }
    }

    /// Forget the sessions that ended.
    fn reap(&mut self) {
        while let Some(ended) = self.tasks.try_join_next() {
            if let Ok(connection) = ended {
                self.stops.remove(&connection);
            }
        }
        // A session that panicked dropped its receiver.
        self.stops.retain(|_, stop| !stop.is_closed());
    }
}

impl IdlePool {
    /// The pool of this process, when IMAP and IDLE are on and credentials
    /// can be opened; `None` otherwise.
    pub fn new(deps: Deps, state: AppState) -> Option<Self> {
        if !deps.imap_enabled() || !deps.imap.config.idle_enabled {
            return None;
        }
        let timing = Timing::of(&deps);
        Some(Self::with_timing(deps, state, timing))
    }

    /// A pool with `timing` (tests).
    pub(crate) fn with_timing(deps: Deps, state: AppState, timing: Timing) -> Self {
        Self {
            shared: Arc::new(Shared {
                deps,
                state,
                holder: Uuid::now_v7(),
                timing,
                asked: AtomicU64::new(0),
            }),
        }
    }

    /// The most sessions it holds.
    fn size(&self) -> usize {
        usize::try_from(self.shared.deps.imap.config.idle_sessions).unwrap_or(usize::MAX)
    }

    /// Hold IDLE sessions until `stop` resolves; then end them and release
    /// the leases.
    pub async fn run(self, stop: impl Future<Output = ()> + Send) {
        tracing::info!(sessions = self.size(), "holding IMAP mailboxes in IDLE");
        let mut held = Held::new();
        let mut ticks = tokio::time::interval(self.shared.timing.renew);
        ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut stop = std::pin::pin!(stop);
        loop {
            tokio::select! {
                () = &mut stop => break,
                Some(_) = held.tasks.join_next(), if !held.tasks.is_empty() => {
                    held.reap();
                    continue;
                }
                _ = ticks.tick() => {}
            }
            self.tick(&mut held).await;
        }
        self.shutdown(held).await;
    }

    /// Renew the leases held (stopping the sessions of those lost), then
    /// claim free ones while there is room.
    pub(crate) async fn tick(&self, held: &mut Held) {
        held.reap();
        let db = &self.shared.state.db;
        let connections = held.connections();
        if connections.is_empty() {
            held.renewed = Instant::now();
        } else {
            match lease::renew(db, self.shared.holder, &connections).await {
                Ok(kept) => {
                    held.renewed = Instant::now();
                    let lost: Vec<ConnectionId> = connections
                        .into_iter()
                        .filter(|connection| !kept.contains(connection))
                        .collect();
                    if !lost.is_empty() {
                        tracing::info!(lost = lost.len(), "IMAP IDLE leases lost");
                    }
                    for connection in lost {
                        held.stop(connection);
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        error = %campbooks_kernel::redact::error(&err),
                        "could not renew the IMAP IDLE leases"
                    );
                    if held.renewed.elapsed() >= self.shared.timing.renew_deadline() {
                        // Another process may claim them soon: stop first.
                        tracing::warn!(
                            sessions = held.stops.len(),
                            "IMAP IDLE leases not renewed in time; stopping every session"
                        );
                        for connection in held.connections() {
                            held.stop(connection);
                        }
                    }
                    return;
                }
            }
        }
        let room = self.size().saturating_sub(held.stops.len());
        let caps = lease::Caps::of(&self.shared.deps.imap.config);
        match lease::claim(db, self.shared.holder, room, caps).await {
            Ok(claimed) => {
                if !claimed.is_empty() {
                    tracing::info!(claimed = claimed.len(), "IMAP IDLE leases claimed");
                }
                for row in claimed {
                    let (stop, stopped) = watch::channel(false);
                    held.stops.insert(row.connection, stop);
                    held.tasks
                        .spawn(hold::hold(Arc::clone(&self.shared), row, stopped));
                }
            }
            Err(err) => tracing::warn!(
                error = %campbooks_kernel::redact::error(&err),
                "could not claim IMAP IDLE leases"
            ),
        }
    }

    /// Stop every session, give them [`SHUTDOWN_GRACE`] to sign out, and
    /// release the leases.
    pub(crate) async fn shutdown(&self, mut held: Held) {
        for connection in held.connections() {
            held.stop(connection);
        }
        let ended = tokio::time::timeout(SHUTDOWN_GRACE, async {
            while held.tasks.join_next().await.is_some() {}
        })
        .await;
        if ended.is_err() {
            held.tasks.abort_all();
        }
        match lease::release(&self.shared.state.db, self.shared.holder).await {
            Ok(released) => tracing::info!(released, "IMAP IDLE leases released"),
            Err(err) => tracing::warn!(
                error = %campbooks_kernel::redact::error(&err),
                "could not release the IMAP IDLE leases; they expire on their own"
            ),
        }
    }

    /// What its sessions share (tests).
    #[cfg(test)]
    pub(crate) fn shared(&self) -> &Shared {
        &self.shared
    }

    /// An empty set of held sessions (tests drive [`tick`](Self::tick)).
    #[cfg(test)]
    pub(crate) fn held() -> Held {
        Held::new()
    }
}
