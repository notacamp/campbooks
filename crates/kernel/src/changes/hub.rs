//! One process's view of the change feed: a single `LISTEN` connection that
//! wakes every open stream of a workspace, and the per-caller stream limit.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, Once, Weak},
    time::Duration,
};

use sqlx::{PgPool, postgres::PgListener};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, watch};

use super::NOTIFY_CHANNEL;
use crate::config::ChangesConfig;

/// Wait before reconnecting a listener that failed.
const LISTEN_RETRY: Duration = Duration::from_secs(5);

/// Streams of one process working in the database at once, whatever the
/// number of open streams: each holds a permit for the whole of a read or a
/// heartbeat (every query of it, one connection at a time), so a wake-up of
/// thousands of streams queues here instead of taking the whole pool.
const CONCURRENT_QUERIES: usize = 4;

/// The process's change-feed hub. Cheap to clone; part of
/// [`AppState`](crate::AppState). Its listener starts with the first stream.
#[derive(Clone)]
pub struct ChangeHub {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for ChangeHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChangeHub")
            .field("workspaces", &lock(&self.inner.workspaces).len())
            .finish_non_exhaustive()
    }
}

struct Inner {
    db: PgPool,
    started: Once,
    /// Dropped with the hub, which stops its listener (and returns the
    /// listener's connection to the pool).
    alive: watch::Sender<()>,
    /// One wake-up channel per workspace with open streams; its value counts
    /// notifications.
    workspaces: Mutex<HashMap<String, watch::Sender<u64>>>,
    /// Open streams per caller (`Principal::subject`).
    streams: Mutex<HashMap<String, usize>>,
    max_streams: usize,
    queries: Arc<Semaphore>,
}

/// Lock a mutex, recovering from a poisoned one (the maps stay consistent:
/// every update is a single insert or remove).
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// One open stream counted against its caller's limit; released on drop.
pub(crate) struct StreamSlot {
    inner: Arc<Inner>,
    subject: String,
}

impl Drop for StreamSlot {
    fn drop(&mut self) {
        let mut streams = lock(&self.inner.streams);
        if let Some(count) = streams.get_mut(&self.subject) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                streams.remove(&self.subject);
            }
        }
    }
}

impl ChangeHub {
    /// The hub for streams over `db`.
    pub fn new(db: PgPool, config: &ChangesConfig) -> Self {
        Self {
            inner: Arc::new(Inner {
                db,
                started: Once::new(),
                alive: watch::channel(()).0,
                workspaces: Mutex::new(HashMap::new()),
                streams: Mutex::new(HashMap::new()),
                max_streams: config.max_streams_per_principal,
                queries: Arc::new(Semaphore::new(CONCURRENT_QUERIES)),
            }),
        }
    }

    /// Count one more stream for `subject`, or `None` when it already holds
    /// the maximum.
    pub(crate) fn admit(&self, subject: &str) -> Option<StreamSlot> {
        let mut streams = lock(&self.inner.streams);
        let count = streams.entry(subject.to_owned()).or_insert(0);
        if *count >= self.inner.max_streams {
            return None;
        }
        *count += 1;
        Some(StreamSlot {
            inner: Arc::clone(&self.inner),
            subject: subject.to_owned(),
        })
    }

    /// Wake-ups for `workspace`'s streams. Starts the listener on first use.
    pub(crate) fn watch(&self, workspace: &str) -> watch::Receiver<u64> {
        self.inner.started.call_once(|| {
            tokio::spawn(listen(
                Arc::downgrade(&self.inner),
                self.inner.db.clone(),
                self.inner.alive.subscribe(),
            ));
        });
        let mut workspaces = lock(&self.inner.workspaces);
        workspaces.retain(|_, sender| sender.receiver_count() > 0);
        workspaces
            .entry(workspace.to_owned())
            .or_insert_with(|| watch::channel(0).0)
            .subscribe()
    }

    /// A permit for one stream's read or heartbeat.
    pub(crate) async fn query_permit(&self) -> Option<OwnedSemaphorePermit> {
        Arc::clone(&self.inner.queries).acquire_owned().await.ok()
    }

    /// Wake `workspace`'s streams, as a notification does.
    pub fn wake(&self, workspace: &str) {
        self.inner.wake(workspace);
    }
}

impl Inner {
    fn wake(&self, workspace: &str) {
        if let Some(sender) = lock(&self.workspaces).get(workspace) {
            sender.send_modify(|n| *n = n.wrapping_add(1));
        }
    }

    /// After the listener (re)connected, notifications may have been missed.
    fn wake_all(&self) {
        for sender in lock(&self.workspaces).values() {
            sender.send_modify(|n| *n = n.wrapping_add(1));
        }
    }
}

/// The listener loop: lives as long as the hub (and its pool), reconnects
/// on failure.
async fn listen(hub: Weak<Inner>, db: PgPool, mut alive: watch::Receiver<()>) {
    loop {
        let connected = tokio::select! {
            connected = connect(&db) => connected,
            _ = alive.changed() => return,
            () = db.close_event() => return,
        };
        let Some(mut listener) = connected else {
            tokio::select! {
                () = tokio::time::sleep(LISTEN_RETRY) => continue,
                _ = alive.changed() => return,
                () = db.close_event() => return,
            }
        };
        if let Some(inner) = hub.upgrade() {
            inner.wake_all();
        }
        loop {
            let received = tokio::select! {
                received = listener.try_recv() => received,
                _ = alive.changed() => return,
                () = db.close_event() => return,
            };
            let Some(inner) = hub.upgrade() else {
                return;
            };
            match received {
                Ok(Some(notification)) => inner.wake(notification.payload()),
                // The connection dropped and was re-established.
                Ok(None) => inner.wake_all(),
                Err(e) => {
                    tracing::warn!(error = %e, "change feed: listener failed; reconnecting");
                    break;
                }
            }
        }
    }
}

/// A listener on [`NOTIFY_CHANNEL`], or `None` (logged) when the database
/// refused.
async fn connect(db: &PgPool) -> Option<PgListener> {
    let mut listener = PgListener::connect_with(db)
        .await
        .inspect_err(
            |e| tracing::warn!(error = %e, "change feed: could not listen for changes; retrying"),
        )
        .ok()?;
    listener
        .listen(NOTIFY_CHANNEL)
        .await
        .inspect_err(
            |e| tracing::warn!(error = %e, "change feed: could not listen for changes; retrying"),
        )
        .ok()?;
    Some(listener)
}
