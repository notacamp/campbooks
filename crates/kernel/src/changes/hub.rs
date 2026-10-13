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

/// How often an idle listener checks that the hub still exists.
const LISTEN_CHECK: Duration = Duration::from_secs(30);

/// Feed queries one process runs at once, whatever the number of open
/// streams: a wake-up of thousands of streams queues here instead of
/// taking the whole connection pool.
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
            tokio::spawn(listen(Arc::downgrade(&self.inner)));
        });
        let mut workspaces = lock(&self.inner.workspaces);
        workspaces.retain(|_, sender| sender.receiver_count() > 0);
        workspaces
            .entry(workspace.to_owned())
            .or_insert_with(|| watch::channel(0).0)
            .subscribe()
    }

    /// A permit to run one feed query.
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

/// The listener loop: lives as long as the hub, reconnects on failure.
async fn listen(hub: Weak<Inner>) {
    loop {
        let Some(db) = hub.upgrade().map(|inner| inner.db.clone()) else {
            return;
        };
        let mut listener = match PgListener::connect_with(&db).await {
            Ok(listener) => listener,
            Err(e) => {
                tracing::warn!(error = %e, "change feed: could not listen for changes; retrying");
                tokio::time::sleep(LISTEN_RETRY).await;
                continue;
            }
        };
        if let Err(e) = listener.listen(NOTIFY_CHANNEL).await {
            tracing::warn!(error = %e, "change feed: could not listen for changes; retrying");
            tokio::time::sleep(LISTEN_RETRY).await;
            continue;
        }
        drop(db);
        if let Some(inner) = hub.upgrade() {
            inner.wake_all();
        }
        loop {
            match tokio::time::timeout(LISTEN_CHECK, listener.try_recv()).await {
                Ok(Ok(Some(notification))) => match hub.upgrade() {
                    Some(inner) => inner.wake(notification.payload()),
                    None => return,
                },
                // The connection dropped and was re-established.
                Ok(Ok(None)) => match hub.upgrade() {
                    Some(inner) => inner.wake_all(),
                    None => return,
                },
                Ok(Err(e)) => {
                    tracing::warn!(error = %e, "change feed: listener failed; reconnecting");
                    tokio::time::sleep(LISTEN_RETRY).await;
                    break;
                }
                Err(_elapsed) => {
                    if hub.strong_count() == 0 {
                        return;
                    }
                }
            }
        }
    }
}
