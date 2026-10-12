//! What an action's handler (or undo handler) can reach.

use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use sqlx::{PgPool, Postgres, Transaction};
use tokio::sync::{Mutex, MutexGuard};

use crate::{AppState, Error, auth::Principal};

/// The transaction of one action call, shared between the kernel (which
/// commits it) and the handler's context (which writes through it).
pub(crate) type SharedTx = Arc<Mutex<Option<Transaction<'static, Postgres>>>>;

/// The context an action's handler and undo handler run in.
///
/// Cheap to clone; it is handed to the handler by value. Grows without
/// breaking handlers: the workspace the call acts in arrives here (with the
/// `workspaces` module) as one more accessor.
///
/// **The action's transaction.** [`tx`](Self::tx) opens the call's
/// transaction on first use (the kernel opens it up front when the call
/// carries an `Idempotency-Key`). The kernel commits it after the handler
/// succeeds, together with the idempotency record and the undo token, and
/// rolls it back if the handler fails. Write through it, and publish the
/// action's events through it, so the change, its events and its undo token
/// exist together or not at all:
///
/// ```ignore
/// async fn archive(ctx: ActionContext, input: ArchiveInput) -> Result<Undoable<Archived, Unarchive>> {
///     let mut tx = ctx.tx().await?;
///     sqlx::query!("UPDATE conversations.conversation SET archived = true WHERE id = $1", input.id)
///         .execute(&mut **tx).await?;
///     ctx.state().events.publish(&mut tx, &ConversationArchived { .. }).await?;
///     Ok(Undoable::new(Archived { .. }, Unarchive { id: input.id }))
/// }
/// ```
///
/// Long calls to other systems do not belong in an action: enqueue a job
/// (in the same transaction) instead.
#[derive(Clone)]
pub struct ActionContext {
    state: AppState,
    principal: Arc<Principal>,
    action: &'static str,
    tx: SharedTx,
}

impl std::fmt::Debug for ActionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActionContext")
            .field("action", &self.action)
            .field("principal", &self.principal.subject)
            .finish_non_exhaustive()
    }
}

impl ActionContext {
    pub(crate) fn new(
        state: AppState,
        principal: Principal,
        action: &'static str,
        tx: SharedTx,
    ) -> Self {
        Self {
            state,
            principal: Arc::new(principal),
            action,
            tx,
        }
    }

    /// The application state: pool, configuration, event bus.
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// The pool, for reads that need not see the action's own writes.
    pub fn pool(&self) -> &PgPool {
        &self.state.db
    }

    /// The authenticated caller.
    pub fn principal(&self) -> &Principal {
        &self.principal
    }

    /// The action being run (`<module>.<verb>`).
    pub fn action(&self) -> &'static str {
        self.action
    }

    /// The action's transaction, opened on first use. Hold the guard only
    /// while you use it; the kernel takes the transaction back when the
    /// handler returns.
    pub async fn tx(&self) -> Result<TxGuard<'_>, Error> {
        let mut slot = self.tx.lock().await;
        if slot.is_none() {
            *slot = Some(self.state.db.begin().await?);
        }
        Ok(TxGuard(slot))
    }
}

/// The action's transaction, borrowed from its [`ActionContext`]: derefs to
/// a `sqlx` [`Transaction`], so `&mut **tx` is the connection to run queries
/// on and `&mut tx` is what `EventBus::publish` takes.
//
// A newtype over the plain `MutexGuard` rather than tokio's
// `MappedMutexGuard`, whose `Send` impl has a `T: 'a` bound that rustc cannot
// prove for every lifetime inside a handler's future ("higher-ranked lifetime
// error"); this guard is `Send` with no such bound.
pub struct TxGuard<'a>(MutexGuard<'a, Option<Transaction<'static, Postgres>>>);

impl std::fmt::Debug for TxGuard<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TxGuard")
    }
}

impl Deref for TxGuard<'_> {
    type Target = Transaction<'static, Postgres>;

    fn deref(&self) -> &Self::Target {
        match &*self.0 {
            Some(tx) => tx,
            None => unreachable!("a TxGuard is only made over an open transaction"),
        }
    }
}

impl DerefMut for TxGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut *self.0 {
            Some(tx) => tx,
            None => unreachable!("a TxGuard is only made over an open transaction"),
        }
    }
}
