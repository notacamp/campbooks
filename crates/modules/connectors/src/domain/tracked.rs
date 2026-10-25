//! The snapshots the module's tracked events carry (ADR 0020), read from its
//! own rows in the transaction of the change, and the plan's enums for its
//! values.
//!
//! Ids, enums, booleans, counts and times only: never an account's address
//! or display name, the provider's account subject, an IMAP host or
//! username, a mailbox key, a folder or label name, a provider's message or
//! event id, a cursor, or a provider's error text. Reasons are the module's
//! own closed codes; one the plan does not name yet is `other`.

use campbooks_kernel::{Error, Result, tenancy::WorkspaceId, tracking::Tracker};
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, Postgres, Transaction};
use uuid::Uuid;

use super::{
    model::{Capability, ConnectionId, Health, MailChoice, Phase, Provider, StreamId},
    outbound::{Outcome, State},
};
use crate::{
    events::{CalendarOutcome, CalendarReason},
    tracking,
};

/// The plan's provider.
pub const fn provider(provider: Provider) -> tracking::Provider {
    match provider {
        Provider::Google => tracking::Provider::Google,
        Provider::Zoho => tracking::Provider::Zoho,
        Provider::Imap => tracking::Provider::Imap,
        Provider::Microsoft => tracking::Provider::Microsoft,
    }
}

/// The plan's capability.
pub const fn capability(capability: Capability) -> tracking::Capability {
    match capability {
        Capability::Messages => tracking::Capability::Messages,
        Capability::Calendar => tracking::Capability::Calendar,
        Capability::Files => tracking::Capability::Files,
        Capability::Bank => tracking::Capability::Bank,
    }
}

/// The plan's health.
pub const fn health(health: Health) -> tracking::Health {
    match health {
        Health::Ok => tracking::Health::Ok,
        Health::NeedsReauth => tracking::Health::NeedsReauth,
        Health::Throttled => tracking::Health::Throttled,
        Health::Failing => tracking::Health::Failing,
    }
}

/// The plan's phase.
pub const fn phase(phase: Phase) -> tracking::Phase {
    match phase {
        Phase::Backfill => tracking::Phase::Backfill,
        Phase::Incremental => tracking::Phase::Incremental,
        Phase::Resync => tracking::Phase::Resync,
    }
}

/// The plan's choice of what becomes of a removed connection's mail.
pub const fn mail_choice(mail: MailChoice) -> tracking::MailChoice {
    match mail {
        MailChoice::Delete => tracking::MailChoice::Delete,
        MailChoice::Keep => tracking::MailChoice::Keep,
    }
}

/// The plan's reason of a connection's health (`other` for a code it does
/// not name yet).
pub fn health_reason(reason: &str) -> tracking::HealthReason {
    match reason {
        "grant_revoked" => tracking::HealthReason::GrantRevoked,
        "access_refused" => tracking::HealthReason::AccessRefused,
        "credentials_unreadable" => tracking::HealthReason::CredentialsUnreadable,
        "unknown_data_centre" => tracking::HealthReason::UnknownDataCentre,
        "imap_settings_missing" => tracking::HealthReason::ImapSettingsMissing,
        "imap_login_refused" => tracking::HealthReason::ImapLoginRefused,
        "rate_limited" => tracking::HealthReason::RateLimited,
        "provider_errors" => tracking::HealthReason::ProviderErrors,
        _ => tracking::HealthReason::Other,
    }
}

/// The plan's kind of a send's failure (`other` for a code it does not name
/// yet).
pub fn send_error(reason: &str) -> tracking::SendError {
    match reason {
        "unavailable" => tracking::SendError::Unavailable,
        "not_connected" => tracking::SendError::NotConnected,
        "needs_reauth" => tracking::SendError::NeedsReauth,
        "too_large" => tracking::SendError::TooLarge,
        "rejected" => tracking::SendError::Rejected,
        "unknown_outcome" => tracking::SendError::UnknownOutcome,
        _ => tracking::SendError::Other,
    }
}

/// The plan's kind of a calendar change; `None` for one it does not name.
pub fn calendar_change(change: &str) -> Option<tracking::CalendarChange> {
    Some(match change {
        "create" => tracking::CalendarChange::Create,
        "update" => tracking::CalendarChange::Update,
        "update_occurrence" => tracking::CalendarChange::UpdateOccurrence,
        "delete" => tracking::CalendarChange::Delete,
        "respond" => tracking::CalendarChange::Respond,
        _ => return None,
    })
}

/// The plan's end of a calendar change.
pub const fn calendar_outcome(outcome: CalendarOutcome) -> tracking::CalendarOutcome {
    match outcome {
        CalendarOutcome::Applied => tracking::CalendarOutcome::Applied,
        CalendarOutcome::Superseded => tracking::CalendarOutcome::Superseded,
        CalendarOutcome::Skipped => tracking::CalendarOutcome::Skipped,
        CalendarOutcome::Failed => tracking::CalendarOutcome::Failed,
    }
}

/// The plan's reason a calendar change did not apply.
pub const fn calendar_reason(reason: CalendarReason) -> tracking::CalendarReason {
    match reason {
        CalendarReason::ChangedAtProvider => tracking::CalendarReason::ChangedAtProvider,
        CalendarReason::Unverified => tracking::CalendarReason::Unverified,
        CalendarReason::NotConnected => tracking::CalendarReason::NotConnected,
        CalendarReason::NeedsReauth => tracking::CalendarReason::NeedsReauth,
        CalendarReason::Rejected => tracking::CalendarReason::Rejected,
        CalendarReason::NotFound => tracking::CalendarReason::NotFound,
        CalendarReason::Unavailable => tracking::CalendarReason::Unavailable,
        CalendarReason::ApiNotEnabled => tracking::CalendarReason::ApiNotEnabled,
        CalendarReason::TooManyAttempts => tracking::CalendarReason::TooManyAttempts,
    }
}

fn send_state(state: &str) -> Result<tracking::SendState> {
    Ok(match state {
        "pending" => tracking::SendState::Pending,
        "sent" => tracking::SendState::Sent,
        "failed" => tracking::SendState::Failed,
        other => return Err(Error::internal(format!("unknown send state `{other}`"))),
    })
}

fn sent_copy(copy: &str) -> Result<tracking::SentCopy> {
    Ok(match copy {
        "pending" => tracking::SentCopy::Pending,
        "appended" => tracking::SentCopy::Appended,
        "found" => tracking::SentCopy::Found,
        "provider" => tracking::SentCopy::Provider,
        "skipped" => tracking::SentCopy::Skipped,
        "failed" => tracking::SentCopy::Failed,
        other => return Err(Error::internal(format!("unknown sent copy `{other}`"))),
    })
}

/// One `connectors.connections` row with its mailbox count, without its
/// address, display name, account subject or server.
type ConnectionRow = (
    Uuid,
    String,
    Uuid,
    Vec<String>,
    String,
    Option<String>,
    bool,
    i64,
    DateTime<Utc>,
    DateTime<Utc>,
);

/// Connection `id` as it is now, from a transaction working in its
/// workspace.
pub async fn connection(
    conn: &mut PgConnection,
    id: ConnectionId,
) -> Result<tracking::ConnectionSnapshot> {
    let row: Option<ConnectionRow> = sqlx::query_as(
        "SELECT c.id, c.provider, c.granted_by, c.capabilities, c.health, c.health_reason,
                c.address_verified,
                (SELECT count(*) FROM connectors.streams s
                  WHERE s.connection_id = c.id AND s.capability = 'messages'),
                c.created_at, c.health_changed_at
           FROM connectors.connections c WHERE c.id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    let (
        id,
        provider_name,
        granted_by,
        capabilities,
        health_name,
        reason,
        address_verified,
        mailboxes,
        created_at,
        health_changed_at,
    ) = row.ok_or_else(|| Error::internal("a tracked connection is gone"))?;
    Ok(tracking::ConnectionSnapshot {
        id,
        provider: provider(Provider::parse(&provider_name)?),
        granted_by,
        capabilities: capabilities
            .iter()
            .map(|name| Capability::parse(name).map(capability))
            .collect::<Result<_>>()?,
        health: health(Health::parse(&health_name)?),
        health_reason: reason.as_deref().map(health_reason),
        address_verified,
        mailboxes,
        created_at,
        health_changed_at,
    })
}

/// The health of connection `id` now, locked for the change about to be
/// made (removal's order: the connection first); `None` when it is gone.
pub async fn health_of(conn: &mut PgConnection, id: ConnectionId) -> Result<Option<Health>> {
    let name: Option<String> = sqlx::query_scalar(
        "SELECT health FROM connectors.connections WHERE id = $1 FOR NO KEY UPDATE",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    name.as_deref().map(Health::parse).transpose()
}

/// One `connectors.streams` row with its connection's provider, without
/// its cursor, mailbox key or address.
type StreamRow = (
    Uuid,
    Uuid,
    String,
    Option<Uuid>,
    String,
    bool,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    String,
);

/// Stream `id` as it is now and its connection's provider, from a
/// transaction working in its workspace.
pub async fn stream(
    conn: &mut PgConnection,
    id: StreamId,
) -> Result<(tracking::StreamSnapshot, tracking::Provider)> {
    let row: Option<StreamRow> = sqlx::query_as(
        "SELECT s.id, s.connection_id, s.capability, s.inbox_id, s.phase, s.paused,
                s.last_synced_at, s.last_change_at, c.provider
           FROM connectors.streams s
           JOIN connectors.connections c ON c.id = s.connection_id
          WHERE s.id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    let (
        id,
        connection_id,
        capability_name,
        inbox_id,
        phase_name,
        paused,
        last_synced_at,
        last_change_at,
        provider_name,
    ) = row.ok_or_else(|| Error::internal("a tracked stream is gone"))?;
    Ok((
        tracking::StreamSnapshot {
            id,
            connection_id,
            capability: capability(Capability::parse(&capability_name)?),
            inbox_id,
            phase: phase(Phase::parse(&phase_name)?),
            paused,
            last_synced_at,
            last_change_at,
        },
        provider(Provider::parse(&provider_name)?),
    ))
}

/// One `connectors.sends` row with its connection's provider, without its
/// document, provider ids or reasons.
type SendRow = (
    Uuid,
    Uuid,
    Option<Uuid>,
    Option<String>,
    String,
    i32,
    Option<String>,
    DateTime<Utc>,
);

/// Send `id` as it is now, from a transaction working in its workspace.
pub async fn send(conn: &mut PgConnection, id: Uuid) -> Result<tracking::SendSnapshot> {
    let row: Option<SendRow> = sqlx::query_as(
        "SELECT s.id, s.inbox_id, s.stream_id, c.provider, s.state, s.attempts, s.sent_copy,
                s.created_at
           FROM connectors.sends s
           LEFT JOIN connectors.streams st ON st.id = s.stream_id
           LEFT JOIN connectors.connections c ON c.id = st.connection_id
          WHERE s.id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    let (id, inbox_id, stream_id, provider_name, state, attempts, copy, created_at) =
        row.ok_or_else(|| Error::internal("a tracked send is gone"))?;
    Ok(tracking::SendSnapshot {
        id,
        inbox_id,
        stream_id,
        provider: provider_name
            .as_deref()
            .map(Provider::parse)
            .transpose()?
            .map(provider),
        state: send_state(&state)?,
        attempts: i64::from(attempts),
        sent_copy: copy.as_deref().map(sent_copy).transpose()?,
        created_at,
    })
}

/// Track, in `tx`, how the outbound changes of `outcome` (a run of `stream`
/// of `connection` at `provider`) ended: one summary, counted by end, when
/// at least one ended (`connectors.outbound_changes_made`). Changes left
/// pending for a later run are not counted.
pub async fn outbound_changes_made(
    tx: &mut Transaction<'_, Postgres>,
    tracker: &Tracker,
    workspace: WorkspaceId,
    (stream, connection): (StreamId, ConnectionId),
    provider_of: Provider,
    outcome: &Outcome,
) -> Result<()> {
    let count = |state: State| {
        i64::try_from(outcome.ends.iter().filter(|end| end.state == state).count())
            .unwrap_or(i64::MAX)
    };
    let (applied, superseded, skipped, failed) = (
        count(State::Applied),
        count(State::Superseded),
        count(State::Skipped),
        count(State::Failed),
    );
    if applied + superseded + skipped + failed == 0 {
        return Ok(());
    }
    tracker
        .track(
            tx,
            &tracking::OutboundChangesMade {
                workspace,
                stream_id: *stream.as_uuid(),
                connection_id: *connection.as_uuid(),
                provider: provider(provider_of),
                applied,
                superseded,
                skipped,
                failed,
            },
        )
        .await?;
    Ok(())
}
