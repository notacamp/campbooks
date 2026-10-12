//! Typed, in-process domain event bus.
//!
//! Modules never call each other; they publish events here and subscribe to
//! the ones they care about. Each event type gets its own broadcast channel,
//! created on first use. Delivery is best-effort and in-memory today (a slow
//! subscriber that falls more than the channel capacity behind sees
//! `RecvError::Lagged`); a durable transport via the job queue comes later
//! behind the same API.

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};

use serde::Serialize;
use tokio::sync::broadcast;

/// A fact a module announces to the rest of the system.
///
/// `NAME` is the stable, dotted wire name (`"mail.message_received"`), used
/// for logs today and for the durable queue later. Never rename it.
pub trait DomainEvent: Serialize + Clone + Send + 'static {
    /// Stable wire name, `<module>.<event>`.
    const NAME: &'static str;
}

/// Default number of events buffered per event type.
pub const DEFAULT_CAPACITY: usize = 1024;

/// The event bus. Cheap to clone; clones share channels.
#[derive(Clone)]
pub struct EventBus {
    capacity: usize,
    channels: Arc<Mutex<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus")
            .field("capacity", &self.capacity)
            .finish_non_exhaustive()
    }
}

impl EventBus {
    /// A bus buffering up to `capacity` events per event type.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            channels: Arc::default(),
        }
    }

    /// Publish an event to every current subscriber of its type. Returns how
    /// many subscribers received it (0 when nobody listens, which is fine).
    pub fn publish<E: DomainEvent>(&self, event: E) -> usize {
        let sent = self.sender::<E>().send(event).unwrap_or(0);
        tracing::debug!(
            event = E::NAME,
            subscribers = sent,
            "domain event published"
        );
        sent
    }

    /// Subscribe to every event of type `E` published from now on.
    pub fn subscribe<E: DomainEvent>(&self) -> broadcast::Receiver<E> {
        self.sender::<E>().subscribe()
    }

    fn sender<E: DomainEvent>(&self) -> broadcast::Sender<E> {
        let mut channels = self.channels.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = channels
            .entry(TypeId::of::<E>())
            .or_insert_with(|| Box::new(broadcast::Sender::<E>::new(self.capacity)));
        match entry.downcast_ref::<broadcast::Sender<E>>() {
            Some(sender) => sender.clone(),
            // Unreachable: the map is keyed by the TypeId of `E`.
            None => broadcast::Sender::new(self.capacity),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Serialize)]
    struct Pinged {
        n: u32,
    }
    impl DomainEvent for Pinged {
        const NAME: &'static str = "test.pinged";
    }

    #[derive(Debug, Clone, PartialEq, Serialize)]
    struct Ponged;
    impl DomainEvent for Ponged {
        const NAME: &'static str = "test.ponged";
    }

    #[tokio::test]
    async fn subscribers_receive_their_type_only() {
        let bus = EventBus::default();
        let mut ping_rx = bus.subscribe::<Pinged>();
        let mut pong_rx = bus.subscribe::<Ponged>();

        assert_eq!(bus.publish(Pinged { n: 1 }), 1);
        assert_eq!(ping_rx.recv().await.unwrap(), Pinged { n: 1 });
        assert!(pong_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn clones_share_channels() {
        let bus = EventBus::default();
        let mut rx = bus.clone().subscribe::<Pinged>();
        bus.publish(Pinged { n: 7 });
        assert_eq!(rx.recv().await.unwrap().n, 7);
    }

    #[test]
    fn publishing_without_subscribers_is_ok() {
        assert_eq!(EventBus::default().publish(Ponged), 0);
    }
}
