//! `connectors.request_changes` ← `conversations.changes_requested`: a
//! member's changes are recorded as outbound changes of the inbox's stream,
//! stamped with Gmail's history id (where they sit in Gmail's order of
//! writes), and the stream runs now to make them
//! (`contracts/conversations.changes_requested.v1.schema.json`).
//!
//! The event is not taken as proof (campbooks-cloud#226): before anything
//! is recorded, or the provider asked for a stamp, the conversations module
//! is asked through the kernel's outbound-requests port which of the
//! changes its conversation still shows. Only those are recorded; a request
//! none of whose changes stand records nothing.
//!
//! Each change recorded is bound to where it is made (campbooks-cloud#242):
//! the stream, its connection and its mailbox, as resolved here. A run made
//! anywhere else (the stream repointed by SQL since) does not make it.

use std::collections::HashSet;

use campbooks_kernel::{
    AppState, Event, EventConsumer, JobContext, JobError, db::BeginFor as _, tenancy::WorkspaceId,
};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    deps::Deps,
    domain::{
        credentials, destinations, marks,
        model::{ConnectionId, Health, InboxId, Provider, StreamId},
        outbound::{self, Mapped, State},
        schedule,
    },
    events::{ChangesRequested, RequestedItem},
    imap, zoho,
};

/// Records a member's changes for the connector to make.
#[derive(Debug, Clone)]
pub struct RequestChanges {
    deps: Deps,
}

impl RequestChanges {
    /// The consumer, composed with the module's dependencies.
    pub fn new(deps: Deps) -> Self {
        Self { deps }
    }
}

/// The stream that feeds an inbox.
struct Target {
    stream: StreamId,
    connection: ConnectionId,
    /// The stream's mailbox at the provider: with the connection, where the
    /// changes are bound to be made (campbooks-cloud#242).
    mailbox: String,
    provider: Provider,
    paused: bool,
    health: Health,
}

impl Target {
    /// Where its changes are made: the connection and mailbox resolved.
    fn at(&self) -> outbound::At<'_> {
        outbound::At {
            connection: self.connection,
            mailbox: &self.mailbox,
        }
    }
}

impl EventConsumer for RequestChanges {
    type Event = ChangesRequested;
    const NAME: &'static str = "connectors.request_changes";
    const QUEUE: &'static str = "events";

    async fn handle(
        &self,
        ctx: &mut JobContext,
        event: Event<ChangesRequested>,
    ) -> Result<(), JobError> {
        let workspace: WorkspaceId = event
            .payload
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("changes_requested without a workspace id"))?;
        let now = self.deps.clock.now();
        let mut tx = ctx.pool().begin_for(workspace).await?;
        // Not a connector's inbox, disconnected meanwhile, or its stream
        // not told apart: nothing to record.
        let Some(target) = target_of(&mut tx, workspace, event.payload.inbox_id).await? else {
            tx.commit().await?;
            return Ok(());
        };
        let request = *event.id.as_uuid();
        let items = standing(ctx.state(), &mut tx, workspace, request, &event.payload).await?;
        if items.is_empty() {
            tx.commit().await?;
            return Ok(());
        }
        let known = known_labels(&mut tx, &target).await?;
        tx.commit().await?;

        let stamp = self.stamp(ctx, workspace, &target).await;
        let binding = ctx.state().binding.clone();
        let tx = ctx.tx().await?;
        // Serialise with the stream's run, which checks for changes recorded
        // while it ran under the same lock.
        sqlx::query!(
            "SELECT id FROM connectors.streams WHERE id = $1 FOR UPDATE",
            target.stream as StreamId
        )
        .fetch_one(&mut **tx)
        .await?;
        let mut pending = 0;
        let mut recorded = Vec::new();
        for item in items {
            let (state, reason, add, remove) =
                outcome(target.provider, &item.change, known.as_ref());
            let id = Uuid::now_v7();
            let inserted = sqlx::query!(
                r#"INSERT INTO connectors.outbound_changes
                       (id, workspace_id, stream_id, request_id, provider_message_id, kind,
                        add_labels, remove_labels, stamp_history_id, state, reason, created_at,
                        done_at)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
                   ON CONFLICT (request_id, provider_message_id) DO NOTHING"#,
                id,
                workspace as WorkspaceId,
                target.stream as StreamId,
                *event.id.as_uuid(),
                item.provider_message_id,
                outbound::kind(&item.change),
                &add,
                &remove,
                stamp,
                state.as_str(),
                reason,
                now,
                (state != State::Pending).then_some(now),
            )
            .execute(&mut **tx)
            .await?
            .rows_affected();
            if inserted > 0 {
                recorded.push(id);
                if state == State::Pending {
                    pending += 1;
                }
            }
        }
        // Each row as written, bound for the run that makes it
        // (campbooks-cloud#227): only rows recorded here, never one found.
        outbound::bind(tx, &binding, workspace, target.at(), &recorded).await?;
        if pending > 0 && !target.paused {
            schedule::run_now(tx, workspace, target.stream, now).await?;
        }
        tracing::info!(%workspace, stream = %target.stream, pending, "outbound changes recorded");
        Ok(())
    }
}

/// The provider's labels `target`'s changes may name (see [`outcome`]).
async fn known_labels(
    conn: &mut PgConnection,
    target: &Target,
) -> Result<Option<HashSet<String>>, JobError> {
    Ok(match target.provider {
        Provider::Google => Some(user_labels(conn, target.stream).await?),
        Provider::Zoho => zoho_labels(conn, target.connection, target.stream).await?,
        Provider::Imap | Provider::Microsoft => None,
    })
}

/// The stream that feeds `inbox` and where its changes are made, read in
/// `conn`: `None` when no stream feeds it, or [`destination_of`] does not
/// agree.
async fn target_of(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    inbox: InboxId,
) -> Result<Option<Target>, JobError> {
    let target = sqlx::query!(
        r#"SELECT s.id AS "stream: StreamId", s.connection_id AS "connection: ConnectionId",
                      c.provider, s.paused, c.health
                 FROM connectors.streams s
                 JOIN connectors.connections c ON c.id = s.connection_id
                WHERE s.inbox_id = $1 AND s.capability = 'messages'"#,
        inbox as InboxId,
    )
    .fetch_optional(&mut *conn)
    .await?;
    let Some(row) = target else {
        return Ok(None);
    };
    let Some((connection, mailbox)) =
        destination_of(conn, workspace, inbox, (row.stream, row.connection)).await?
    else {
        return Ok(None);
    };
    Ok(Some(Target {
        stream: row.stream,
        connection,
        mailbox,
        provider: Provider::parse(&row.provider)?,
        paused: row.paused,
        health: Health::parse(&row.health)?,
    }))
}

/// Where `inbox`'s changes are made, which they are bound to
/// (campbooks-cloud#242): its one stream's connection and mailbox, as
/// [`destinations::of_inbox`] resolves them. `None` (a WARN) when that is
/// not `stream` with `connection` (what the target was read as), or the
/// inbox does not resolve to one stream only (two claiming it are SQL's).
async fn destination_of(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    inbox: InboxId,
    (stream, connection): (StreamId, ConnectionId),
) -> Result<Option<(ConnectionId, String)>, JobError> {
    let at = destinations::of_inbox(conn, inbox).await?.filter(|at| {
        at.destination.stream == *stream.as_uuid()
            && at.destination.connection == *connection.as_uuid()
    });
    if at.is_none() {
        tracing::warn!(
            %workspace,
            %inbox,
            "the inbox's stream could not be told apart; its changes are not recorded"
        );
    }
    Ok(at.map(|at| {
        (
            ConnectionId::from_uuid(at.destination.connection),
            at.destination.mailbox,
        )
    }))
}

/// How `change` is recorded for `provider` (`known`: the provider's user
/// labels other than stream marks; `None` for a Zoho grant without the
/// labels scope): its state, the reason when it is not pending, and the
/// labels to add and remove. Defence in depth: only the flag labels,
/// categories, stream marks and user labels (Gmail), read, flag, inbox,
/// labels and stream marks (Zoho), or read, flag, inbox, trash, spam and
/// stream marks (IMAP), are ever written.
fn outcome(
    provider: Provider,
    change: &crate::events::RequestedChange,
    known: Option<&HashSet<String>>,
) -> (State, Option<&'static str>, Vec<String>, Vec<String>) {
    let writable = |label: &str| match provider {
        Provider::Google => outbound::writable(label),
        Provider::Zoho => zoho::state::writable(label),
        Provider::Imap => imap::write::writable(label),
        // Nothing is written to Microsoft yet (campbooks-cloud#317).
        Provider::Microsoft => false,
    };
    let mapped = match provider {
        Provider::Google => outbound::map(change, known.unwrap_or(&HashSet::new())),
        Provider::Zoho => outbound::map_zoho(change, known),
        Provider::Imap => outbound::map_imap(change),
        Provider::Microsoft => Mapped::Skip("unsupported"),
    };
    match mapped {
        Mapped::Labels { add, remove } if !add.iter().chain(&remove).all(|l| writable(l)) => {
            (State::Skipped, Some("unsupported"), Vec::new(), Vec::new())
        }
        Mapped::Labels { add, remove } => (State::Pending, None, add, remove),
        Mapped::Skip(reason) => (State::Skipped, Some(reason), Vec::new(), Vec::new()),
    }
}

/// The labels named like stream marks among `labels` (id, name): never a
/// label change's.
fn mark_ids(labels: &[(String, String)]) -> HashSet<&str> {
    labels
        .iter()
        .filter(|(_, name)| marks::of_name(name).is_some())
        .map(|(id, _)| id.as_str())
        .collect()
}

/// The user labels Gmail has for `stream`, by key, stream marks left out.
async fn user_labels(
    conn: &mut PgConnection,
    stream: StreamId,
) -> Result<HashSet<String>, JobError> {
    let named: Vec<(String, String)> = sqlx::query_as(
        "SELECT label_id, name FROM connectors.gmail_labels
          WHERE stream_id = $1 AND kind = 'user'",
    )
    .bind(stream)
    .fetch_all(&mut *conn)
    .await?;
    let marks = mark_ids(&named);
    Ok(sqlx::query_scalar!(
        "SELECT label_id FROM connectors.gmail_labels WHERE stream_id = $1 AND kind = 'user'",
        stream as StreamId
    )
    .fetch_all(conn)
    .await?
    .into_iter()
    .filter(|id| !marks.contains(id.as_str()))
    .collect())
}

/// The labels Zoho has for `stream`, by id, stream marks left out; `None`
/// when `connection`'s grant has no labels scope (granted before it was
/// asked for).
async fn zoho_labels(
    conn: &mut PgConnection,
    connection: ConnectionId,
    stream: StreamId,
) -> Result<Option<HashSet<String>>, JobError> {
    let scopes: String =
        sqlx::query_scalar("SELECT scopes FROM connectors.connections WHERE id = $1")
            .bind(connection)
            .fetch_one(&mut *conn)
            .await?;
    if !zoho::labels_granted(&scopes) {
        return Ok(None);
    }
    let named: Vec<(String, String)> =
        sqlx::query_as("SELECT label_id, name FROM connectors.zoho_labels WHERE stream_id = $1")
            .bind(stream)
            .fetch_all(conn)
            .await?;
    let marks = mark_ids(&named);
    Ok(Some(
        named
            .iter()
            .map(|(id, _)| id.as_str())
            .filter(|id| !marks.contains(id))
            .map(ToOwned::to_owned)
            .collect(),
    ))
}

/// The items of `request` (event `id`) the conversations module recorded
/// for it, as it answers in `conn` (the outbound-requests port); the others
/// are logged and left out.
async fn standing<'r>(
    state: &AppState,
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    id: uuid::Uuid,
    request: &'r ChangesRequested,
) -> Result<Vec<&'r RequestedItem>, JobError> {
    let asked: Vec<serde_json::Value> = request
        .items
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<_, _>>()
        .map_err(|e| {
            JobError::fatal(format!(
                "encoding requested changes: {}",
                campbooks_kernel::redact::json(&e)
            ))
        })?;
    let standing = state
        .outbound
        .changes_standing(
            state,
            conn,
            workspace,
            id,
            request.inbox_id.into(),
            request.conversation,
            &asked,
        )
        .await?;
    let items: Vec<&RequestedItem> = request
        .items
        .iter()
        .zip(standing.into_iter().chain(std::iter::repeat(false)))
        .filter_map(|(item, stands)| stands.then_some(item))
        .collect();
    if items.len() < request.items.len() {
        tracing::warn!(
            %workspace,
            asked = request.items.len(),
            standing = items.len(),
            "changes_requested names changes the conversations module did not request; those are not made"
        );
    }
    Ok(items)
}

impl RequestChanges {
    /// Gmail's history id now (1 quota unit), or `None` when Gmail cannot
    /// be asked: the change then applies in arrival order.
    async fn stamp(
        &self,
        ctx: &JobContext,
        workspace: WorkspaceId,
        target: &Target,
    ) -> Option<i64> {
        // Zoho keeps no history: its changes apply in arrival order.
        let google = self.deps.google.as_ref()?;
        if target.provider != Provider::Google || target.health == Health::NeedsReauth {
            return None;
        }
        let rate = self.deps.config.google.units_per_second;
        if !matches!(
            schedule::take_budget(ctx.pool(), target.provider, 1, rate).await,
            Ok(Ok(()))
        ) {
            return None;
        }
        let token = credentials::access_token(ctx.pool(), &self.deps, workspace, target.connection)
            .await
            .ok()?;
        match google.gmail(&token).profile().await {
            Ok(profile) => profile.history_id.parse().ok(),
            Err(err) => {
                tracing::info!(stream = %target.stream, error = %err, "no history stamp for outbound changes");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests;
