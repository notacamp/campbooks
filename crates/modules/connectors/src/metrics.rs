//! The module's metrics (`openspec/specs/observability`): sync runs by
//! provider and outcome, and the streams the dispatcher finds overdue.
//! Attributes are small enums only, never ids.

use std::sync::OnceLock;

use opentelemetry::{
    KeyValue,
    metrics::{Counter, Gauge},
};

use crate::domain::model::Provider;

/// How a sync run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncOutcome {
    /// It read (and applied) a page.
    Ok,
    /// It waits: the provider throttled it, or its budget is spent.
    Throttled,
    /// The provider refused its grant or token.
    Refused,
    /// The provider or the run failed.
    Failed,
}

impl SyncOutcome {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Throttled => "throttled",
            Self::Refused => "refused",
            Self::Failed => "failed",
        }
    }
}

struct Instruments {
    syncs: Counter<u64>,
    overdue: Gauge<u64>,
}

/// Built on first use, from the process's meter provider (installed before
/// any job runs).
fn instruments() -> &'static Instruments {
    static INSTRUMENTS: OnceLock<Instruments> = OnceLock::new();
    INSTRUMENTS.get_or_init(|| {
        let meter = campbooks_kernel::telemetry::meter();
        Instruments {
            syncs: meter
                .u64_counter("campbooks.connector.syncs")
                .with_description("Sync runs of connected accounts, by provider and outcome.")
                .build(),
            overdue: meter
                .u64_gauge("campbooks.connector.overdue_streams")
                .with_description(
                    "Streams due for more than five minutes that the dispatcher had not yet run.",
                )
                .build(),
        }
    })
}

/// Count a sync run of `provider` that ended with `outcome`.
pub fn sync_finished(provider: Provider, outcome: SyncOutcome) {
    instruments().syncs.add(
        1,
        &[
            KeyValue::new("provider", provider.as_str()),
            KeyValue::new("outcome", outcome.as_str()),
        ],
    );
}

/// Record how many streams were overdue at a dispatcher tick.
pub fn overdue_streams(count: u64) {
    instruments().overdue.record(count, &[]);
}
