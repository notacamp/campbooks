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

use campbooks_kernel::tenancy::WorkspaceId;

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
    /// The longest one renewal or claim may take: well within a renewal
    /// period, so the watchdog's margin holds.
    pub db_timeout: Duration,
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
            db_timeout: Duration::from_secs(20),
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

/// One session of a pool.
#[derive(Debug)]
pub(crate) struct Running {
    /// Its stop signal (closed once its task has ended).
    stop: watch::Sender<bool>,
    /// Its connection's workspace.
    workspace: WorkspaceId,
    /// Asked to stop, and still signing out.
    stopping: bool,
}

/// The sessions a pool holds now.
#[derive(Debug)]
pub(crate) struct Held {
    /// Each session, by its connection, until its task ends.
    sessions: HashMap<ConnectionId, Running>,
    /// The sessions' tasks.
    tasks: JoinSet<ConnectionId>,
    /// When the last renewal that succeeded was asked for.
    renewed: Instant,
}

impl Held {
    fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            tasks: JoinSet::new(),
            renewed: Instant::now(),
        }
    }

    /// The connections held now (not stopping).
    pub(crate) fn connections(&self) -> Vec<ConnectionId> {
        self.sessions
            .iter()
            .filter(|(_, running)| !running.stopping)
            .map(|(connection, _)| *connection)
            .collect()
    }

    /// Whether any session is held (not stopping).
    fn holding(&self) -> bool {
        self.sessions.values().any(|running| !running.stopping)
    }

    /// Each workspace's sessions, stopping ones included (they still hold a
    /// connection to their server).
    fn per_workspace(&self) -> HashMap<WorkspaceId, usize> {
        let mut counts = HashMap::new();
        for running in self.sessions.values() {
            *counts.entry(running.workspace).or_default() += 1;
        }
        counts
    }

    /// Stop `connection`'s session; it counts until its task ends.
    fn stop(&mut self, connection: ConnectionId) {
        if let Some(running) = self.sessions.get_mut(&connection) {
            running.stopping = true;
            running.stop.send_replace(true);
        }
    }

    /// Stop every session.
    fn stop_all(&mut self) {
        for connection in self.connections() {
            self.stop(connection);
        }
    }

    /// Forget the sessions whose task ended (however it ended: a task that
    /// ends drops its receiver).
    fn reap(&mut self) {
        while self.tasks.try_join_next().is_some() {}
        self.sessions.retain(|_, running| !running.stop.is_closed());
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
        self.run_with(Held::new(), stop).await;
    }

    /// [`run`](Self::run), holding `held` already.
    ///
    /// Besides the renewals, a watchdog stops every session once the last
    /// renewal that succeeded was asked for longer ago than the renewal
    /// deadline, whatever a tick is doing: every database call is bounded
    /// ([`Timing::db_timeout`]) and the leases outlive the deadline by a
    /// renewal period, so no session outlives its lease.
    pub(crate) async fn run_with(self, mut held: Held, stop: impl Future<Output = ()> + Send) {
        let mut ticks = tokio::time::interval(self.shared.timing.renew);
        ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let deadline = self.shared.timing.renew_deadline();
        let mut stop = std::pin::pin!(stop);
        loop {
            tokio::select! {
                biased;
                () = &mut stop => break,
                () = tokio::time::sleep_until(held.renewed + deadline), if held.holding() => {
                    tracing::warn!(
                        sessions = held.connections().len(),
                        "IMAP IDLE leases not renewed in time; stopping every session"
                    );
                    held.stop_all();
                    continue;
                }
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
    /// claim free ones while there is room. Each database call takes at
    /// most [`Timing::db_timeout`].
    pub(crate) async fn tick(&self, held: &mut Held) {
        held.reap();
        let db = &self.shared.state.db;
        let bound = self.shared.timing.db_timeout;
        // Stamped before asking: the lease runs from when the database
        // renewed it, which is no earlier.
        let asked_at = Instant::now();
        let connections = held.connections();
        if connections.is_empty() {
            held.renewed = asked_at;
        } else {
            let renewed =
                tokio::time::timeout(bound, lease::renew(db, self.shared.holder, &connections))
                    .await
                    .unwrap_or_else(|_| Err(campbooks_kernel::Error::internal("timed out")));
            match renewed {
                Ok(kept) => {
                    held.renewed = asked_at;
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
                            sessions = held.connections().len(),
                            "IMAP IDLE leases not renewed in time; stopping every session"
                        );
                        held.stop_all();
                    }
                    return;
                }
            }
        }
        // Sessions still signing out count: they hold a connection still.
        let room = self.size().saturating_sub(held.sessions.len());
        let caps = lease::Caps::of(&self.shared.deps.imap.config);
        let claimed = tokio::time::timeout(
            bound,
            lease::claim(db, self.shared.holder, room, caps, &held.per_workspace()),
        )
        .await
        .unwrap_or_else(|_| Err(campbooks_kernel::Error::internal("timed out")));
        match claimed {
            Ok(claimed) => {
                if !claimed.is_empty() {
                    tracing::info!(claimed = claimed.len(), "IMAP IDLE leases claimed");
                }
                for row in claimed {
                    let (stop, stopped) = watch::channel(false);
                    held.sessions.insert(
                        row.connection,
                        Running {
                            stop,
                            workspace: row.workspace,
                            stopping: false,
                        },
                    );
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
        held.stop_all();
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
