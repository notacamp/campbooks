//! `connectors.sync_stream` — one run of one stream: refresh the token if
//! needed, take budget, read one page through the adapter, then in the
//! job's transaction apply the adapter's index updates, store and announce
//! the batch (`connectors.messages_changed`), advance the cursor and set the
//! next run.
//!
//! A run first makes the stream's pending outbound changes at the provider
//! (`gmail::write`), records how each ended, and only then reads: pushes
//! and pulls of one stream never interleave, and share its budget, backoff
//! and health.
//!
//! The stream row is the schedule: provider trouble (throttling, outages)
//! is recorded on it and the run ends normally, with the next run set by
//! the backoff, rather than failing the job.
//!
//! An IMAP stream (`imap::read`) reads over one session to its server,
//! holding one of the host's few session slots, and keeps where each
//! message is (`connectors.imap_locations`) with the index. Its pending
//! changes are made over that session first (`imap::write`), then the
//! copies in Sent of messages sent through its SMTP server (`imap::sent`),
//! and how they ended (the locations and index they leave) recorded before
//! it reads. While its connection's address is checked
//! (`domain::address_checks`), the run looks for the check's message in the
//! Inbox before it reads (`imap::check`), and the stream runs at the poll's
//! shortest interval.

mod calendar;

use campbooks_kernel::{
    JobContext, JobError,
    blobs::BlobOwner,
    db::BeginFor as _,
    jobs::{EnqueueOptions, JobHandler},
    redact,
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, TimeDelta, Utc};
use secrecy::SecretString;
use sqlx::{PgPool, Postgres, Transaction};

use std::{collections::HashMap, time::Duration};

use crate::{
    contract::{BATCH_CONTENT_TYPE, MessageChange, MessageChangeBatch, ProviderStream},
    deps::Deps,
    domain::{
        address_checks::{self, Looked, Waiting},
        credentials::{self, TokenError},
        emit,
        health::{self, LOGIN_REFUSALS},
        imap_servers,
        marks::MarkIds,
        model::{ConnectionId, Health, InboxId, Phase, Provider, SendId, StreamId},
        outbound::{self, Backlog},
        push::Push,
        removal::BATCH_OWNER,
        schedule::{self, LEASE, Rate, SYNC_STREAM, SyncStreamPayload},
    },
    features::imap_idle::lease,
    gmail::{self, Reader},
    google::LabelInfo,
    imap::{
        self, Refusal,
        check::{Found, Look},
        folders::Role,
        read::{Failure, Page},
        sent::{self, Pending},
        session::Session,
        state::{Cursor, LocationOp},
    },
    mail::{IndexOp, ReadError},
    metrics::{self, SyncOutcome},
    provider::ProviderError,
    zoho,
};

/// Runs `connectors.sync_stream`.
#[derive(Debug, Clone)]
pub struct SyncStream {
    deps: Deps,
}

impl SyncStream {
    /// The job handler, composed with the module's dependencies.
    pub fn new(deps: Deps) -> Self {
        Self { deps }
    }
}

/// A stream and its connection, as a run needs them.
#[derive(Debug)]
struct Target {
    stream: StreamId,
    connection: ConnectionId,
    provider: Provider,
    granted_by: UserId,
    health: Health,
    provider_server: Option<String>,
    mailbox_key: String,
    inbox: Option<InboxId>,
    phase: Phase,
    cursor: serde_json::Value,
    interval_seconds: i32,
    paused: bool,
    consecutive_failures: i32,
    throttled_until: Option<DateTime<Utc>>,
    /// The grant has Zoho's labels scope (Zoho only).
    labels_granted: bool,
}

impl JobHandler for SyncStream {
    type Payload = SyncStreamPayload;
    const NAME: &'static str = SYNC_STREAM;
    const QUEUE: &'static str = "sync";
    const MAX_ATTEMPTS: u32 = 3;

    async fn run(&self, ctx: &mut JobContext, payload: SyncStreamPayload) -> Result<(), JobError> {
        let workspace: WorkspaceId = ctx
            .workspace()
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| JobError::fatal("sync enqueued outside a workspace"))?;
        let now = self.deps.clock.now();
        let Some(target) = load(ctx, workspace, payload.stream).await? else {
            return Ok(()); // Disconnected meanwhile.
        };
        if target.paused || target.health == Health::NeedsReauth {
            return Ok(());
        }
        if let Some(until) = target.throttled_until.filter(|until| *until > now) {
            schedule::set_due(ctx.tx().await?, workspace, target.stream, until).await?;
            return Ok(());
        }
        if is_calendar(ctx, workspace, target.stream).await? {
            return self.run_calendar(ctx, workspace, &target, now).await;
        }
        let configured = match target.provider {
            Provider::Google => self.deps.google.is_some(),
            Provider::Zoho => self.deps.zoho.is_some(),
            Provider::Imap if self.deps.imap.config.enabled && self.deps.cipher.is_some() => true,
            Provider::Imap => return self.idle_imap(ctx, workspace, &target, now).await,
        };
        let (true, Some(inbox)) = (configured, target.inbox) else {
            // The provider is no longer configured (or the stream has no
            // adapter yet): the leased schedule row brings it back later.
            tracing::debug!(stream = %target.stream, "no adapter for this stream");
            return Ok(());
        };
        if target.provider == Provider::Imap {
            // As for the other providers: stale changes given up first, then
            // only changes bound to where this run makes them.
            let retention = TimeDelta::days(i64::from(self.deps.config.batch_retention_days));
            outbound::expire(ctx.pool(), workspace, target.stream, now - retention, now).await?;
            let backlog = outbound::backlog(
                ctx.pool(),
                &ctx.state().binding,
                workspace,
                (target.stream, target.at()),
                now,
            )
            .await?;
            let read = self
                .run_imap(ctx, workspace, &target, &backlog, now)
                .await
                .inspect_err(|_| metrics::sync_finished(target.provider, SyncOutcome::Failed))?;
            return match read {
                Some(read) => {
                    self.finish(ctx, workspace, &target, inbox, read, backlog.last(), now)
                        .await
                }
                None => Ok(()),
            };
        }

        let token =
            match credentials::access_token(ctx.pool(), &self.deps, workspace, target.connection)
                .await
            {
                Ok(token) => token,
                Err(TokenError::NeedsReauth(reason)) => {
                    metrics::sync_finished(target.provider, SyncOutcome::Refused);
                    return self
                        .needs_reauth(ctx, workspace, &target, reason, now)
                        .await;
                }
                Err(TokenError::Provider(err)) => {
                    return self.trouble(ctx, workspace, &target, &err, now).await;
                }
                Err(TokenError::Database(err)) => return Err(err.into()),
            };

        // Stale changes are given up before anything is taken: never made
        // late after a long pause.
        let retention = TimeDelta::days(i64::from(self.deps.config.batch_retention_days));
        outbound::expire(ctx.pool(), workspace, target.stream, now - retention, now).await?;
        // Only changes bound to where this run makes them: the connection
        // and mailbox loaded above (campbooks-cloud#242).
        let backlog = outbound::backlog(
            ctx.pool(),
            &ctx.state().binding,
            workspace,
            (target.stream, target.at()),
            now,
        )
        .await?;
        let read = match target.provider {
            Provider::Google => {
                self.run_gmail(ctx, workspace, &target, &token, &backlog, now)
                    .await
            }
            Provider::Zoho => {
                self.run_zoho(ctx, workspace, &target, &token, &backlog, now)
                    .await
            }
            Provider::Imap => Ok(None),
        }
        .inspect_err(|_| metrics::sync_finished(target.provider, SyncOutcome::Failed))?;
        // Without a page the run waits or has recorded its trouble, and
        // counted it.
        let Some(read) = read else {
            return Ok(());
        };
        self.finish(ctx, workspace, &target, inbox, read, backlog.last(), now)
            .await
    }
}

/// What a run read, whatever the provider: applied in the job's
/// transaction.
#[derive(Debug)]
struct RunPage {
    changes: Vec<MessageChange>,
    index: Vec<IndexOp>,
    /// A fresh Gmail `labels.list`.
    labels: Option<Vec<LabelInfo>>,
    /// A fresh listing of the Zoho account's labels.
    zoho_labels: Option<Vec<zoho::ZohoLabel>>,
    /// The cursor after the page, as stored.
    cursor: serde_json::Value,
    /// The cursor as the event carries it.
    token: String,
    phase: Phase,
    more: bool,
    /// Gmail's history id the run read up to (closes open write windows).
    read_up_to: Option<i64>,
    /// Where IMAP messages now are (IMAP only).
    imap: Vec<LocationOp>,
    /// What the run offers for IDLE (IMAP only).
    idle: Option<lease::Offer>,
    /// The connection's address is being checked: the next run is at the
    /// poll's shortest interval (IMAP only).
    checking: bool,
}

impl SyncStream {
    /// The number of distinct messages `backlog` touches.
    fn messages(backlog: &Backlog) -> usize {
        backlog
            .pending
            .iter()
            .map(|p| p.provider_message_id.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len()
    }

    /// Record a push's outcome on its own (the writes happened, whatever
    /// the read does next), with the stream marks it created in `provider`'s
    /// label cache. Returns the trouble that stopped it.
    async fn record_push(
        ctx: &JobContext,
        workspace: WorkspaceId,
        target: &Target,
        provider: Provider,
        pushed: Push,
        now: DateTime<Utc>,
    ) -> Result<Option<ProviderError>, JobError> {
        let stream = target.stream;
        let mut tx = ctx.pool().begin_for(workspace).await?;
        outbound::record_outcome(
            &mut tx,
            &ctx.state().binding,
            workspace,
            (stream, target.at()),
            &pushed.outcome,
            now,
        )
        .await?;
        for (id, name) in &pushed.created {
            let insert = match provider {
                // Gmail's own record (`kind`) says what it is.
                Provider::Google => sqlx::query(
                    "INSERT INTO connectors.gmail_labels (workspace_id, stream_id, label_id, name, kind)
                     VALUES ($1, $2, $3, $4, 'user')
                     ON CONFLICT (stream_id, label_id) DO UPDATE SET name = EXCLUDED.name",
                ),
                Provider::Zoho => sqlx::query(
                    "INSERT INTO connectors.zoho_labels
                         (workspace_id, stream_id, label_id, name, refreshed_at)
                     VALUES ($1, $2, $3, $4, now())
                     ON CONFLICT (stream_id, label_id) DO UPDATE SET name = EXCLUDED.name",
                ),
                // IMAP has no label cache (its stream folders are in the cursor).
                Provider::Imap => continue,
            };
            insert
                .bind(workspace)
                .bind(stream)
                .bind(id)
                .bind(name)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(pushed.trouble)
    }

    /// Wait for `wait` before the stream's next run.
    async fn wait_for_budget(
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        stream: StreamId,
        wait: std::time::Duration,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let at = now + TimeDelta::from_std(wait).unwrap_or(TimeDelta::seconds(1));
        schedule::set_due(ctx.tx().await?, workspace, stream, at).await?;
        Ok(())
    }

    /// A Gmail run: budget, push, read. `None` once the run has ended
    /// without a page (waiting, trouble recorded).
    #[allow(
        clippy::too_many_lines,
        reason = "one run read top to bottom: budget, push, report, read"
    )]
    async fn run_gmail(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        token: &str,
        backlog: &Backlog,
        now: DateTime<Utc>,
    ) -> Result<Option<RunPage>, JobError> {
        let Some(google) = self.deps.google.clone() else {
            return Ok(None);
        };
        let cursor: gmail::Cursor = serde_json::from_value(target.cursor.clone()).map_err(|e| {
            JobError::fatal(format!(
                "stream {} has an unreadable cursor: {}",
                target.stream,
                redact::json(&e)
            ))
        })?;
        let labels = if backlog.pending.is_empty() {
            Vec::new()
        } else {
            label_rows(ctx, workspace, target.stream, Provider::Google).await?
        };
        let marks =
            MarkIds::from_labels(labels.iter().map(|(id, name)| (id.as_str(), name.as_str())));
        let mut reported = Vec::new();
        // The push's planned units are taken now and settled with what it
        // spent once it is done (campbooks-cloud#252).
        let writes = if backlog.pending.is_empty() {
            0
        } else {
            gmail::write::units(backlog, &marks)
        };
        let cost = gmail::UNITS_PER_RUN + writes;
        let rate = self.deps.config.google.units_per_second;
        if let Err(wait) = schedule::take_budget(ctx.pool(), target.provider, cost, rate).await? {
            metrics::sync_finished(target.provider, SyncOutcome::Throttled);
            Self::wait_for_budget(ctx, workspace, target.stream, wait, now).await?;
            return Ok(None);
        }
        if backlog.last().is_some() {
            // The index holds what was read up to the cursor's history id.
            let read_from = cursor
                .history_id
                .as_deref()
                .and_then(|h| h.parse::<i64>().ok());
            let pushed = gmail::write::push(&google.gmail(token), backlog, &marks, read_from).await;
            let names = named(&labels, &pushed);
            reported = stream_changed(backlog, &pushed, |labels| {
                gmail::normalize::flags(labels, &names).provider_stream
            });
            // The read keeps the units it reserved; the writes pay what
            // they spent.
            let spent = gmail::UNITS_PER_RUN.saturating_add(pushed.spent);
            // What Gmail now holds is recorded first; settling the budget
            // after is best effort (a miss only leaves the budget as taken).
            let trouble =
                Self::record_push(ctx, workspace, target, Provider::Google, pushed, now).await?;
            if let Err(err) =
                schedule::settle_budget(ctx.pool(), target.provider, cost, spent, rate).await
            {
                tracing::warn!(
                    %workspace,
                    stream = %target.stream,
                    error = %campbooks_kernel::redact::error(&err),
                    "could not settle the Gmail budget after a push"
                );
            }
            if let Some(err) = trouble {
                self.trouble(ctx, workspace, target, &err, now).await?;
                return Ok(None);
            }
        }
        let state = ctx.state().clone();
        let reader = Reader {
            gmail: google.gmail(token),
            pool: ctx.pool(),
            blobs: &state.blobs,
            workspace,
            stream: target.stream,
            phase: target.phase,
            cursor,
            now,
            backfill_days: self.deps.config.gmail.backfill_days,
            max_message_bytes: self.deps.config.gmail.max_message_bytes,
            run_budget_bytes: self.deps.config.gmail.run_budget_bytes,
        };
        let mut page = match reader.read().await {
            Ok(page) => page,
            Err(ReadError::Provider(err)) => {
                self.trouble(ctx, workspace, target, &err, now).await?;
                return Ok(None);
            }
            Err(other) => return Err(JobError::retry(other.to_string())),
        };
        if !reported.is_empty() {
            let names = gmail_names(ctx, workspace, target.stream).await?;
            let reporting = Reporting {
                ids: &reported,
                marks: MarkIds::from_labels(names.iter().map(|(id, n)| (id.as_str(), n.as_str()))),
                label: str::to_owned,
            };
            report(
                ctx,
                workspace,
                target.stream,
                &reporting,
                &mut page.changes,
                |labels| gmail::normalize::flags(labels, &names),
            )
            .await?;
        }
        Ok(Some(RunPage {
            cursor: serde_json::to_value(&page.cursor)
                .map_err(|e| JobError::fatal(format!("encoding a cursor: {}", redact::json(&e))))?,
            token: page.cursor.token(),
            read_up_to: page
                .cursor
                .history_id
                .as_deref()
                .and_then(|h| h.parse::<i64>().ok()),
            changes: page.changes,
            index: page.index,
            labels: page.labels,
            zoho_labels: None,
            imap: Vec::new(),
            idle: None,
            checking: false,
            phase: page.phase,
            more: page.more,
        }))
    }

    /// A Zoho run: the data centre, the login's budget, push, read. `None`
    /// once the run has ended without a page.
    #[allow(
        clippy::too_many_lines,
        reason = "one run read top to bottom: budget, push, report, read"
    )]
    async fn run_zoho(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        token: &str,
        backlog: &Backlog,
        now: DateTime<Utc>,
    ) -> Result<Option<RunPage>, JobError> {
        let Some(zoho) = self.deps.zoho.clone() else {
            return Ok(None);
        };
        let Some(centre) = target
            .provider_server
            .as_deref()
            .and_then(|server| zoho.centre(server))
        else {
            self.needs_reauth(ctx, workspace, target, "unknown_data_centre", now)
                .await?;
            return Ok(None);
        };
        let cursor: zoho::read::Cursor =
            serde_json::from_value(target.cursor.clone()).map_err(|e| {
                JobError::fatal(format!(
                    "stream {} has an unreadable cursor: {}",
                    target.stream,
                    redact::json(&e)
                ))
            })?;
        let config = &self.deps.config.zoho;
        // Labels and stream marks only for a grant with the labels scope.
        let rows = if target.labels_granted {
            label_rows(ctx, workspace, target.stream, Provider::Zoho).await?
        } else {
            Vec::new()
        };
        // Label reads are reserved as expected and settled as made.
        let label_calls = zoho::read::label_requests(target.labels_granted, target.phase, &rows);
        let cost = zoho::read::requests_per_run(cursor.folders.len())
            + label_calls
            + zoho::write::requests(Self::messages(backlog));
        let key = schedule::connection_budget(target.connection);
        let rate = Rate::per_minute(config.requests_per_minute);
        if let Err(wait) = schedule::take(ctx.pool(), &key, cost, rate).await? {
            metrics::sync_finished(target.provider, SyncOutcome::Throttled);
            Self::wait_for_budget(ctx, workspace, target.stream, wait, now).await?;
            return Ok(None);
        }
        let mail = zoho.mail(&centre, token, &target.mailbox_key);
        let labels = target
            .labels_granted
            .then(|| zoho::read::LabelCache::new(rows.iter().cloned().collect()));
        let mut reported = Vec::new();
        if backlog.last().is_some() {
            let listed: Vec<&str> = cursor.folders.iter().map(|f| f.id.as_str()).collect();
            let folders = zoho::write::Folders {
                inbox: cursor.inbox(),
                trash: cursor.folder_of_kind(zoho::state::FolderKind::Trash),
                spam: cursor.folder_of_kind(zoho::state::FolderKind::Spam),
                listed: &listed,
            };
            let marks =
                MarkIds::from_labels(rows.iter().map(|(id, name)| (id.as_str(), name.as_str())));
            let pushed = zoho::write::push(&mail, backlog, &folders, &marks).await;
            if target.labels_granted {
                let names = named(&rows, &pushed);
                reported = stream_changed(backlog, &pushed, |tokens| {
                    zoho::state::flags_with(tokens, &names).provider_stream
                });
            }
            if let Some(err) =
                Self::record_push(ctx, workspace, target, Provider::Zoho, pushed, now).await?
            {
                self.trouble(ctx, workspace, target, &err, now).await?;
                return Ok(None);
            }
        }
        let state = ctx.state().clone();
        let reader = zoho::read::Reader {
            mail,
            pool: ctx.pool(),
            blobs: &state.blobs,
            workspace,
            stream: target.stream,
            phase: target.phase,
            cursor,
            now,
            config,
            labels: labels.as_ref(),
        };
        let mut page = match reader.read().await {
            Ok(page) => page,
            Err(ReadError::Provider(err)) => {
                self.trouble(ctx, workspace, target, &err, now).await?;
                return Ok(None);
            }
            Err(other) => return Err(JobError::retry(other.to_string())),
        };
        if let Some(cache) = labels.as_ref()
            && let Err(err) =
                schedule::settle(ctx.pool(), &key, label_calls, cache.calls(), rate).await
        {
            tracing::warn!(
                %workspace,
                stream = %target.stream,
                error = %campbooks_kernel::redact::error(&err),
                "could not settle the Zoho budget for label reads"
            );
        }
        if let Some(cache) = labels.as_ref().filter(|_| !reported.is_empty()) {
            let names = cache.names();
            let reporting = Reporting {
                ids: &reported,
                marks: MarkIds::from_labels(names.iter().map(|(id, n)| (id.as_str(), n.as_str()))),
                label: zoho::state::label_token,
            };
            report(
                ctx,
                workspace,
                target.stream,
                &reporting,
                &mut page.changes,
                |tokens| zoho::state::flags_with(tokens, &names),
            )
            .await?;
        }
        Ok(Some(RunPage {
            cursor: serde_json::to_value(&page.cursor)
                .map_err(|e| JobError::fatal(format!("encoding a cursor: {}", redact::json(&e))))?,
            token: page.cursor.token(),
            read_up_to: None,
            changes: page.changes,
            index: page.index,
            labels: None,
            zoho_labels: labels.as_ref().and_then(zoho::read::LabelCache::fresh),
            imap: Vec::new(),
            idle: None,
            checking: false,
            phase: page.phase,
            more: page.more,
        }))
    }

    /// An IMAP stream on an instance that does not read IMAP (off, or no
    /// instance secret): its password is opened, so one that no longer
    /// opens shows as `needs_reauth`, and it waits at the slowest interval.
    /// No server is contacted.
    async fn idle_imap(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        if let Some(cipher) = self.deps.cipher.as_deref() {
            let mut tx = ctx.pool().begin_for(workspace).await?;
            let opened = credentials::password(&mut tx, cipher, target.connection).await;
            tx.commit().await?;
            match opened {
                Err(TokenError::NeedsReauth(reason)) => {
                    metrics::sync_finished(target.provider, SyncOutcome::Refused);
                    return self.needs_reauth(ctx, workspace, target, reason, now).await;
                }
                Err(TokenError::Database(err)) => return Err(err.into()),
                Err(TokenError::Provider(_)) | Ok(_) => {}
            }
        }
        let idle = now + TimeDelta::seconds(i64::from(self.deps.config.poll.max_seconds));
        schedule::set_due(ctx.tx().await?, workspace, target.stream, idle).await?;
        Ok(())
    }

    /// An IMAP run: the account and its password, one of its host's
    /// session slots, then over one session `backlog`'s changes and one
    /// page. `None` once the run has ended without a page (waiting, trouble
    /// recorded).
    #[allow(
        clippy::too_many_lines,
        reason = "one run read top to bottom: account, slot, read, outcome"
    )]
    async fn run_imap(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        backlog: &Backlog,
        now: DateTime<Utc>,
    ) -> Result<Option<RunPage>, JobError> {
        let Some(cipher) = self.deps.cipher.clone() else {
            return Ok(None);
        };
        let mut tx = ctx.pool().begin_for(workspace).await?;
        let account = imap_servers::imap_server(&mut tx, target.connection).await?;
        let address: Option<String> =
            sqlx::query_scalar("SELECT address FROM connectors.streams WHERE id = $1")
                .bind(target.stream)
                .fetch_optional(&mut *tx)
                .await?
                .flatten();
        let opened = credentials::password(&mut tx, &cipher, target.connection).await;
        let copies = pending_copies(&mut tx, target.stream).await?;
        let waiting = address_checks::waiting(&mut tx, target.connection).await?;
        tx.commit().await?;
        let check = waiting.and_then(|waiting| {
            let domain = address.as_deref()?.rsplit_once('@')?.1.to_owned();
            let code = address_checks::code(&ctx.state().signer, target.connection, waiting.nonce);
            Some(CheckRun {
                waiting,
                search: format!("{}{code}", address_checks::PREFIX),
                message_id: address_checks::message_id(&code, &domain),
            })
        });
        let checking = check.is_some();
        let password = match opened {
            Ok(password) => password,
            Err(TokenError::NeedsReauth(reason)) => {
                metrics::sync_finished(target.provider, SyncOutcome::Refused);
                self.needs_reauth(ctx, workspace, target, reason, now)
                    .await?;
                return Ok(None);
            }
            Err(TokenError::Provider(err)) => {
                self.trouble(ctx, workspace, target, &err, now).await?;
                return Ok(None);
            }
            Err(TokenError::Database(err)) => return Err(err.into()),
        };
        let Some((username, server)) = account else {
            metrics::sync_finished(target.provider, SyncOutcome::Refused);
            self.needs_reauth(ctx, workspace, target, "imap_settings_missing", now)
                .await?;
            return Ok(None);
        };
        let cursor: Cursor = serde_json::from_value(target.cursor.clone()).map_err(|e| {
            JobError::fatal(format!(
                "stream {} has an unreadable cursor: {}",
                target.stream,
                redact::json(&e)
            ))
        })?;
        let imap = self.deps.imap.clone();
        let provider_host = imap::is_provider_host(&server.host);
        let signer = &ctx.state().signer;
        let host_key = lease::host_key(signer, &server.host);
        let offer = lease::Offer {
            login_key: lease::login_key(signer, &username, &server.host),
            host_key: host_key.clone(),
            provider_host,
            capable: false,
        };
        // IDLE sessions hold some of the host's sessions for long: the run
        // needs a slot beyond them.
        let idling = if imap.config.idle_enabled {
            lease::live_on_host(ctx.pool(), &host_key, workspace).await?
        } else {
            (0, 0)
        };
        let slots = run_slots(&imap.config, &server.host, workspace, provider_host, idling);
        if !take_slots(ctx.pool(), &slots).await? {
            // As many sessions as it may: the next dispatch.
            metrics::sync_finished(target.provider, SyncOutcome::Throttled);
            let wait = Duration::from_secs(u64::from(self.deps.config.poll.min_seconds));
            Self::wait_for_budget(ctx, workspace, target.stream, wait, now).await?;
            return Ok(None);
        }
        let state = ctx.state().clone();
        let run = ImapRun {
            imap: &imap,
            server: &server,
            username: &username,
            password: &password,
            pool: ctx.pool(),
            blobs: &state.blobs,
            binding: &state.binding,
            workspace,
            connection: target.connection,
            stream: target.stream,
            at: target.at(),
            backlog,
            phase: target.phase,
            cursor,
            now,
            address,
            copies,
            check,
        };
        let read = tokio::time::timeout(IMAP_RUN_LIMIT, Box::pin(read_imap(run)))
            .await
            .unwrap_or(Err(ImapTrouble::Failed(Failure::Server("imap_timed_out"))));
        give_back_slots(ctx.pool(), &slots).await;
        match read {
            Ok(page) => {
                let offer = lease::Offer {
                    // After the backfill only: its continuation runs must
                    // not be doubled by IDLE's.
                    capable: imap.config.idle_enabled
                        && page.cursor.idle == Some(true)
                        && page.phase == Phase::Incremental,
                    ..offer
                };
                let mut page = imap_page(page)?;
                page.idle = Some(offer);
                page.checking = checking;
                Ok(Some(page))
            }
            Err(ImapTrouble::Refused(refusal)) => {
                self.imap_refused(ctx, workspace, target, refusal, now)
                    .await?;
                Ok(None)
            }
            Err(ImapTrouble::Failed(Failure::Busy)) => {
                self.imap_busy(ctx, workspace, target, now).await?;
                Ok(None)
            }
            Err(ImapTrouble::Failed(Failure::Server(code))) => {
                let err = ProviderError::Unavailable(code.to_owned());
                self.trouble(ctx, workspace, target, &err, now).await?;
                Ok(None)
            }
            Err(ImapTrouble::Failed(Failure::Read(other))) => {
                Err(JobError::retry(other.to_string()))
            }
        }
    }

    /// The IMAP server would not let the run in: a refused password counts
    /// towards [`LOGIN_REFUSALS`] (then the connection needs the member); a
    /// busy server slows the stream down; anything else is a failed run.
    async fn imap_refused(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        refusal: Refusal,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        match refusal {
            Refusal::ImapLoginRefused => {
                // Counted with the IDLE sessions' refusals (one count).
                let tx = ctx.tx().await?;
                let refusals =
                    health::count_login_refusal(tx, target.connection, target.stream).await?;
                if refusals >= LOGIN_REFUSALS {
                    metrics::sync_finished(target.provider, SyncOutcome::Refused);
                    return self
                        .needs_reauth(ctx, workspace, target, "imap_login_refused", now)
                        .await;
                }
                let err = ProviderError::Unavailable(refusal.code().to_owned());
                self.trouble(ctx, workspace, target, &err, now).await
            }
            Refusal::ImapBusy => self.imap_busy(ctx, workspace, target, now).await,
            other => {
                let err = ProviderError::Unavailable(other.code().to_owned());
                self.trouble(ctx, workspace, target, &err, now).await
            }
        }
    }

    /// The IMAP server said it is busy (IMAP has no `Retry-After`): throttle
    /// the stream for twice its interval.
    async fn imap_busy(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let poll = &self.deps.config.poll;
        let wait = i64::from(target.interval_seconds)
            .saturating_mul(2)
            .clamp(i64::from(poll.min_seconds), i64::from(poll.max_seconds));
        let retry_after = Some(Duration::from_secs(u64::try_from(wait).unwrap_or(60)));
        self.trouble(
            ctx,
            workspace,
            target,
            &ProviderError::RateLimited { retry_after },
            now,
        )
        .await
    }

    /// The grant stopped working (`reason`).
    async fn needs_reauth(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        reason: &'static str,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let events = ctx.state().events.clone();
        health::needs_reauth(
            ctx.tx().await?,
            &events,
            &workspace.to_string(),
            target.connection,
            target.granted_by,
            reason,
            now,
        )
        .await?;
        Ok(())
    }
}

impl SyncStream {
    /// Apply a page and count the run.
    #[allow(
        clippy::too_many_arguments,
        reason = "one call site per provider; the values are distinct"
    )]
    async fn finish(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        inbox: InboxId,
        read: RunPage,
        pushed_up_to: Option<uuid::Uuid>,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let applied = self
            .apply(ctx, workspace, target, inbox, read, pushed_up_to, now)
            .await;
        let outcome = if applied.is_ok() {
            SyncOutcome::Ok
        } else {
            SyncOutcome::Failed
        };
        metrics::sync_finished(target.provider, outcome);
        applied
    }

    /// Apply a page in the job's transaction. `pushed_up_to` is the newest
    /// outbound change this run made: newer ones arrived during the run and
    /// call for another at once.
    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "one call site; the values are distinct; one transaction read top to bottom"
    )]
    async fn apply(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        inbox: InboxId,
        mut page: RunPage,
        pushed_up_to: Option<uuid::Uuid>,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let state = ctx.state().clone();
        let key = workspace.to_string();
        // Mail the first sync read, or received before it completed, is
        // never mirrored: it goes in a batch of its own, marked `backfill`.
        let synced = synced_at(&mut page.cursor, page.phase, now);
        let first_sync = target.phase == Phase::Backfill;
        let documents = split(inbox, page.changes, &page.index, first_sync, synced);
        let changed = !documents.is_empty();
        let change_count: usize = documents.iter().map(|d| d.changes.len()).sum();
        // Blobs are stored before the transaction that references them.
        let mut batches = Vec::new();
        for document in &documents {
            let bytes = serde_json::to_vec(document)
                .map_err(|e| JobError::fatal(format!("encoding a batch: {}", redact::json(&e))))?;
            let blob = state.blobs.put(&key, bytes, BATCH_CONTENT_TYPE).await?;
            batches.push((blob, document.changes.len()));
        }
        let cursor_json = page.cursor;
        let interval =
            schedule::next_interval(target.interval_seconds, changed, &self.deps.config.poll);
        let retention = TimeDelta::days(i64::from(self.deps.config.batch_retention_days));

        let tx = ctx.tx().await?;
        // The connection before the stream (removal's order).
        health::lock(tx, target.connection).await?;
        if let Some(labels) = &page.labels {
            replace_labels(tx, workspace, target.stream, labels).await?;
        }
        if let Some(labels) = &page.zoho_labels {
            replace_zoho_labels(tx, workspace, target.stream, labels, now).await?;
        }
        emit::apply_index(tx, &state, workspace, target.stream, page.index).await?;
        imap::state::apply(tx, workspace, target.stream, page.imap).await?;
        outbound::forget_left_trash(tx, target.stream).await?;
        for (batch, count) in &batches {
            emit::store_batch(
                tx,
                &state,
                workspace,
                target.stream,
                inbox,
                batch,
                &page.token,
                *count,
                now,
            )
            .await?;
        }
        prune_batches(tx, &state, workspace, target.stream, now - retention).await?;
        sqlx::query!(
            r#"UPDATE connectors.streams
                  SET phase = $2, cursor = $3, interval_seconds = $4, last_synced_at = $5,
                      last_change_at = CASE WHEN $6 THEN $5 ELSE last_change_at END,
                      consecutive_failures = 0, throttled_until = NULL
                WHERE id = $1"#,
            target.stream as StreamId,
            page.phase.as_str(),
            cursor_json,
            interval,
            now,
            changed,
        )
        .execute(&mut **tx)
        .await?;
        if matches!(target.health, Health::Throttled | Health::Failing) {
            health::set(tx, target.connection, Health::Ok, None, now).await?;
        }
        let least_wait = self
            .offer_idle(tx, workspace, target, page.idle.as_ref(), now)
            .await?;
        if settle_outbound(
            tx,
            target.stream,
            pushed_up_to,
            now - retention,
            page.read_up_to,
        )
        .await?
        {
            page.more = true;
        }
        if page.more {
            // More is waiting: run again straight away, holding the
            // schedule row so the dispatcher does not double it.
            schedule::set_due(tx, workspace, target.stream, now + LEASE).await?;
            // Ordered after this run; no unique key, since this run (which
            // may itself be a continuation) still holds it.
            campbooks_kernel::jobs::enqueue_to::<Self>(
                tx,
                &SyncStreamPayload {
                    stream: target.stream,
                },
                EnqueueOptions::for_workspace(workspace.to_string())
                    .ordered_by(format!("{SYNC_STREAM}:{}", target.stream)),
            )
            .await?;
        } else {
            let mut next = now + TimeDelta::seconds(i64::from(interval.max(least_wait)));
            if page.checking {
                // An address check waits for its message: look again soon.
                let soon = i64::from(self.deps.config.poll.min_seconds);
                next = next.min(now + TimeDelta::seconds(soon));
            }
            schedule::set_due(tx, workspace, target.stream, next).await?;
        }
        tracing::info!(
            %workspace, stream = %target.stream, changes = change_count,
            phase = page.phase.as_str(), more = page.more, "stream synced"
        );
        Ok(())
    }

    /// An IMAP stream offers itself for IDLE (`offer`), or withdraws, in the
    /// run's transaction. The least wait before its next poll: held in
    /// IDLE, the safety poll's; none otherwise.
    async fn offer_idle(
        &self,
        tx: &mut Transaction<'static, Postgres>,
        workspace: WorkspaceId,
        target: &Target,
        offer: Option<&lease::Offer>,
        now: DateTime<Utc>,
    ) -> Result<i32, JobError> {
        let Some(offer) = offer else {
            return Ok(0);
        };
        let config = &self.deps.imap.config;
        let quiet = TimeDelta::days(i64::from(config.idle_quiet_days));
        let held = lease::offer(
            tx,
            workspace,
            target.connection,
            target.stream,
            offer,
            now - quiet,
        )
        .await?;
        Ok(if held {
            i32::try_from(config.idle_poll_seconds).unwrap_or(i32::MAX)
        } else {
            0
        })
    }

    /// The provider refused the access token: drop it so the next run
    /// refreshes it, backing off; a grant refused run after run although it
    /// refreshes needs the member (`needs_reauth`, `access_refused`).
    async fn refused(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let poll = &self.deps.config.poll;
        // The access token was refused: drop it so the next run
        // refreshes it (and finds out if the grant is gone). A
        // grant refused run after run although it refreshes needs
        // the member.
        let failures = target.consecutive_failures.saturating_add(1);
        if u32::try_from(failures).unwrap_or(u32::MAX) >= self.deps.config.failing_after {
            let events = ctx.state().events.clone();
            health::needs_reauth(
                ctx.tx().await?,
                &events,
                &workspace.to_string(),
                target.connection,
                target.granted_by,
                "access_refused",
                now,
            )
            .await?;
            return Ok(());
        }
        let backoff = i64::from(poll.min_seconds)
            .saturating_mul(1_i64 << failures.clamp(0, 16))
            .min(i64::from(poll.max_seconds));
        let tx = ctx.tx().await?;
        health::lock(tx, target.connection).await?;
        credentials::forget_access_token(tx, target.connection).await?;
        sqlx::query!(
            "UPDATE connectors.streams SET consecutive_failures = $2 WHERE id = $1",
            target.stream as StreamId,
            failures,
        )
        .execute(&mut **tx)
        .await?;
        schedule::set_due(
            tx,
            workspace,
            target.stream,
            now + TimeDelta::seconds(backoff),
        )
        .await?;
        Ok(())
    }

    /// The provider failed: record it on the stream and the connection,
    /// and set the next run.
    async fn trouble(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        err: &ProviderError,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let poll = &self.deps.config.poll;
        let outcome = match err {
            ProviderError::Unauthorized | ProviderError::InvalidGrant => SyncOutcome::Refused,
            ProviderError::RateLimited { .. } => SyncOutcome::Throttled,
            ProviderError::NotFound
            | ProviderError::Unavailable(_)
            | ProviderError::Unexpected(_) => SyncOutcome::Failed,
        };
        metrics::sync_finished(target.provider, outcome);
        match err {
            ProviderError::Unauthorized => self.refused(ctx, workspace, target, now).await,
            ProviderError::InvalidGrant => {
                let events = ctx.state().events.clone();
                health::needs_reauth(
                    ctx.tx().await?,
                    &events,
                    &workspace.to_string(),
                    target.connection,
                    target.granted_by,
                    "grant_revoked",
                    now,
                )
                .await?;
                Ok(())
            }
            ProviderError::RateLimited { retry_after } => {
                let wait = retry_after
                    .and_then(|d| TimeDelta::from_std(d).ok())
                    .unwrap_or(TimeDelta::seconds(i64::from(poll.min_seconds)));
                let until = now + wait;
                let tx = ctx.tx().await?;
                health::lock(tx, target.connection).await?;
                sqlx::query!(
                    "UPDATE connectors.streams SET throttled_until = $2 WHERE id = $1",
                    target.stream as StreamId,
                    until,
                )
                .execute(&mut **tx)
                .await?;
                health::set(
                    tx,
                    target.connection,
                    Health::Throttled,
                    Some("rate_limited"),
                    now,
                )
                .await?;
                schedule::set_due(tx, workspace, target.stream, until).await?;
                tracing::info!(%workspace, stream = %target.stream, %until, "throttled by the provider");
                Ok(())
            }
            ProviderError::NotFound
            | ProviderError::Unavailable(_)
            | ProviderError::Unexpected(_) => {
                let failures = target.consecutive_failures.saturating_add(1);
                // Back off from the minimum, doubling per failure, up to
                // the maximum interval.
                let max = i64::from(poll.max_seconds);
                let backoff = i64::from(poll.min_seconds)
                    .saturating_mul(1_i64 << failures.clamp(0, 16))
                    .min(max);
                let tx = ctx.tx().await?;
                health::lock(tx, target.connection).await?;
                sqlx::query!(
                    "UPDATE connectors.streams SET consecutive_failures = $2 WHERE id = $1",
                    target.stream as StreamId,
                    failures,
                )
                .execute(&mut **tx)
                .await?;
                if u32::try_from(failures).unwrap_or(u32::MAX) >= self.deps.config.failing_after {
                    health::set(
                        tx,
                        target.connection,
                        Health::Failing,
                        Some("provider_errors"),
                        now,
                    )
                    .await?;
                }
                schedule::set_due(
                    tx,
                    workspace,
                    target.stream,
                    now + TimeDelta::seconds(backoff),
                )
                .await?;
                tracing::warn!(%workspace, stream = %target.stream, failures, error = %err, "sync run failed");
                Ok(())
            }
        }
    }
}

/// The longest an IMAP run may take in all, well within a lease.
const IMAP_RUN_LIMIT: Duration = Duration::from_mins(10);

/// Seconds an IMAP host's session slots take to refill whole: a slot a
/// process died holding comes back within a lease.
const SLOT_SECONDS: f64 = 900.0;

/// The budget of `host`'s IMAP sessions (`connectors.provider_budgets`).
fn imap_slots(host: &str) -> String {
    format!("imap:{host}")
}

/// The budget of `workspace`'s sessions to `host`: its share of the host's.
fn imap_workspace_slots(host: &str, workspace: WorkspaceId) -> String {
    format!("imap:{host}:{workspace}")
}

/// One session budget a run takes a slot of: its key, its rate, and the
/// slots it must leave to live IDLE sessions.
type Slot = (String, Rate, u32);

/// The slots a run on `host` takes: the workspace's share of the host, then
/// the host's. A known provider's host holds
/// `provider_sessions_per_host` sessions, any other `sessions_per_host`;
/// `idling` (the host's live IDLE sessions, and the workspace's) are left
/// free on the host, and on a small host in the workspace's share too.
fn run_slots(
    config: &crate::config::ImapConfig,
    host: &str,
    workspace: WorkspaceId,
    provider_host: bool,
    idling: (u32, u32),
) -> [Slot; 2] {
    let (on_host, own) = idling;
    let sessions = if provider_host {
        config.provider_sessions_per_host
    } else {
        config.sessions_per_host
    };
    [
        (
            imap_workspace_slots(host, workspace),
            imap_rate(config.workspace_sessions_per_host),
            if provider_host { 0 } else { own },
        ),
        (imap_slots(host), imap_rate(sessions), on_host),
    ]
}

/// Take one slot of each of `slots` (key, rate, reserve), in order; none
/// when one is full (those taken are given back). Whether they were taken.
async fn take_slots(pool: &PgPool, slots: &[Slot]) -> Result<bool, JobError> {
    for (n, (key, rate, reserve)) in slots.iter().enumerate() {
        if schedule::take_leaving(pool, key, 1, *rate, *reserve)
            .await?
            .is_err()
        {
            give_back_slots(pool, &slots[..n]).await;
            return Ok(false);
        }
    }
    Ok(true)
}

/// Give back one slot of each of `slots` (best effort: a slot not given
/// back comes back as its bucket refills).
async fn give_back_slots(pool: &PgPool, slots: &[Slot]) {
    for (key, rate, _) in slots {
        if let Err(err) = schedule::settle(pool, key, 1, 0, *rate).await {
            tracing::warn!(
                error = %campbooks_kernel::redact::error(&err),
                "could not give back an IMAP session slot"
            );
        }
    }
}

/// `sessions` slots per host: taken one per run, given back when it ends.
fn imap_rate(sessions: u32) -> Rate {
    Rate {
        per_second: f64::from(sessions) / SLOT_SECONDS,
        burst_seconds: SLOT_SECONDS,
    }
}

/// What one IMAP run writes and reads with.
struct ImapRun<'a> {
    imap: &'a imap::Imap,
    server: &'a imap::Server,
    username: &'a str,
    password: &'a SecretString,
    pool: &'a PgPool,
    blobs: &'a campbooks_kernel::blobs::BlobStore,
    binding: &'a campbooks_kernel::binding::Binding,
    workspace: WorkspaceId,
    connection: ConnectionId,
    stream: StreamId,
    /// Where the stream's changes are made (their binding, #242).
    at: outbound::At<'a>,
    /// The changes to make before reading.
    backlog: &'a Backlog,
    phase: Phase,
    cursor: Cursor,
    now: DateTime<Utc>,
    address: Option<String>,
    /// The copies in Sent still to be made (`imap::sent`), and the sends
    /// whose copy cannot be read back.
    copies: Copies,
    /// The address check whose message to look for, if one waits.
    check: Option<CheckRun>,
}

/// An address check a run looks for: the check, and what its message's
/// `Message-ID` is searched by and must be.
struct CheckRun {
    waiting: Waiting,
    search: String,
    message_id: String,
}

/// The copies in Sent a run makes: those it can, and the sends whose
/// recorded copy is unreadable (failed without contacting the server).
#[derive(Debug, Default)]
struct Copies {
    pending: Vec<Pending>,
    broken: Vec<SendId>,
}

/// The copies in Sent stream `stream` still has to make, oldest first, at
/// most [`sent::COPIES_PER_RUN`].
async fn pending_copies(
    conn: &mut sqlx::PgConnection,
    stream: StreamId,
) -> Result<Copies, JobError> {
    let rows: Vec<CopyRow> = sqlx::query_as(
        "SELECT s.id, s.provider_message_id, s.sent_copy_raw, s.sent_copy_attempts,
                m.rfc_message_id
           FROM connectors.sends s
           LEFT JOIN connectors.messages m
             ON m.stream_id = s.stream_id AND m.provider_message_id = s.provider_message_id
          WHERE s.stream_id = $1 AND s.sent_copy = 'pending'
          ORDER BY s.updated_at, s.id
          LIMIT $2",
    )
    .bind(stream)
    .bind(sent::COPIES_PER_RUN)
    .fetch_all(conn)
    .await?;
    let mut copies = Copies::default();
    for (id, key, raw, attempts, rfc_message_id) in rows {
        let send = SendId::from_uuid(id);
        let raw = raw.and_then(|raw| serde_json::from_value(raw).ok());
        match (key, raw) {
            (Some(key), Some(raw)) => copies.pending.push(Pending {
                send,
                key,
                rfc_message_id,
                raw,
                attempts,
            }),
            _ => copies.broken.push(send),
        }
    }
    Ok(copies)
}

/// A pending copy's row: send, index key, bytes, attempts, `Message-ID`.
type CopyRow = (
    uuid::Uuid,
    Option<String>,
    Option<serde_json::Value>,
    i32,
    Option<String>,
);

/// Why an IMAP run read nothing.
#[derive(Debug)]
enum ImapTrouble {
    /// The server would not let it in.
    Refused(Refusal),
    /// It failed once in.
    Failed(Failure),
}

/// Sign in, make the pending changes, read one page, sign out.
async fn read_imap(run: ImapRun<'_>) -> Result<Page, ImapTrouble> {
    let opening = Session::open(run.imap, run.server, run.username, run.password);
    let mut session = tokio::time::timeout(run.imap.timeout(), opening)
        .await
        .unwrap_or(Err(Refusal::ImapUnreachable))
        .map_err(ImapTrouble::Refused)?;
    let mut cursor = run.cursor.clone();
    if !run.backlog.pending.is_empty() {
        match push_imap(&run, &mut session).await {
            Ok(after) => cursor = after,
            Err(trouble) => {
                session.logout().await;
                return Err(trouble);
            }
        }
    }
    // An address check's message before the read: once found and removed,
    // it is never read in.
    if let Some(check) = &run.check
        && let Err(trouble) = check_imap(&run, &mut session, check, &cursor).await
    {
        session.logout().await;
        return Err(trouble);
    }
    let page = imap::read::Reader {
        session: &mut session,
        pool: run.pool,
        blobs: run.blobs,
        workspace: run.workspace,
        stream: run.stream,
        phase: run.phase,
        cursor,
        now: run.now,
        config: &run.imap.config,
        address: run.address.clone(),
    }
    .read()
    .await;
    // Copies in Sent once the read is done: one that will not go never holds
    // up inbound mail.
    if let Ok(read) = &page
        && (!run.copies.pending.is_empty() || !run.copies.broken.is_empty())
    {
        copy_imap(&run, &mut session, &read.cursor).await;
    }
    session.logout().await;
    page.map_err(ImapTrouble::Failed)
}

/// Make `run`'s pending changes over `session` and record how they ended,
/// with the locations, index and cursor they leave, before anything is
/// read (so the read never takes them for the server's). The cursor the
/// read starts from; trouble that stopped the push once it is recorded.
async fn push_imap(run: &ImapRun<'_>, session: &mut Session) -> Result<Cursor, ImapTrouble> {
    let pushed = imap::write::Writer {
        session,
        pool: run.pool,
        workspace: run.workspace,
        stream: run.stream,
        cursor: run.cursor.clone(),
        config: &run.imap.config,
    }
    .push(run.backlog)
    .await
    .map_err(|err| ImapTrouble::Failed(Failure::Read(err)))?;
    let trouble = pushed.trouble;
    let cursor = record_imap_push(run, pushed)
        .await
        .map_err(|err| ImapTrouble::Failed(Failure::Read(err)))?;
    match trouble {
        None => Ok(cursor),
        Some(imap::write::Trouble::Busy) => Err(ImapTrouble::Failed(Failure::Busy)),
        Some(other) => Err(ImapTrouble::Failed(Failure::Server(other.code()))),
    }
}

/// Record, in one transaction of the stream's workspace, how `pushed`'s
/// changes ended and the index tokens, locations and cursor they leave.
async fn record_imap_push(
    run: &ImapRun<'_>,
    pushed: imap::write::Pushed,
) -> Result<Cursor, ReadError> {
    let mut tx = run.pool.begin_for(run.workspace).await?;
    // The connection before the stream (removal's order).
    health::lock(&mut tx, run.connection).await?;
    outbound::record_outcome(
        &mut tx,
        run.binding,
        run.workspace,
        (run.stream, run.at),
        &pushed.push.outcome,
        run.now,
    )
    .await?;
    imap::state::apply(&mut tx, run.workspace, run.stream, pushed.locations).await?;
    let cursor = serde_json::to_value(&pushed.cursor)
        .map_err(|e| ReadError::Corrupt(format!("encoding a cursor: {}", redact::json(&e))))?;
    sqlx::query("UPDATE connectors.streams SET cursor = cursor || $2 WHERE id = $1")
        .bind(run.stream)
        .bind(cursor)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(pushed.cursor)
}

/// Look for `check`'s message in the Inbox over `session` and record what
/// was found (`imap::check`): found, the address is verified. A busy server
/// waits for the next run; a cut session, or one that expunged more than it
/// was asked to, is the run's trouble.
async fn check_imap(
    run: &ImapRun<'_>,
    session: &mut Session,
    check: &CheckRun,
    cursor: &Cursor,
) -> Result<(), ImapTrouble> {
    let found = Look {
        session,
        search: &check.search,
        message_id: &check.message_id,
        trash: cursor.folder_of(Role::Trash).map(|f| f.name.as_str()),
        timeout: run.imap.timeout(),
    }
    .run()
    .await;
    let looked = match found {
        Ok(Found::Yes(_) | Found::Hostile) => Looked::Found,
        Ok(Found::No) => Looked::NotFound,
        Err(imap::write::Trouble::Busy) => return Ok(()),
        Err(trouble) => return Err(ImapTrouble::Failed(Failure::Server(trouble.code()))),
    };
    if let Err(err) = address_checks::record(
        run.pool,
        run.workspace,
        run.connection,
        check.waiting,
        looked,
        run.now,
    )
    .await
    {
        tracing::warn!(
            stream = %run.stream,
            error = %redact::error(&err),
            "what an address check found could not be recorded"
        );
    }
    if found == Ok(Found::Hostile) {
        return Err(ImapTrouble::Failed(Failure::Server("imap_expunged_more")));
    }
    Ok(())
}

/// Make `run`'s copies in Sent over `session`, each recorded as it ends
/// (`imap::sent`), after the read. Trouble is logged, never the run's: the
/// copies left wait for the next run.
async fn copy_imap(run: &ImapRun<'_>, session: &mut Session, cursor: &Cursor) {
    let copied = sent::Appender {
        session,
        pool: run.pool,
        blobs: run.blobs,
        workspace: run.workspace,
        connection: run.connection,
        stream: run.stream,
        cursor,
        config: &run.imap.config,
    }
    .append(run.copies.pending.clone(), &run.copies.broken)
    .await;
    match copied {
        Ok(copied) => tracing::info!(
            stream = %run.stream,
            appended = copied.appended,
            trouble = copied.trouble.map_or("", imap::write::Trouble::code),
            "copies made in Sent"
        ),
        Err(err) => tracing::warn!(
            stream = %run.stream,
            error = %redact::error(&err),
            "the copies made in Sent could not be recorded"
        ),
    }
}

/// An IMAP page as the run applies it.
fn imap_page(page: Page) -> Result<RunPage, JobError> {
    Ok(RunPage {
        cursor: serde_json::to_value(&page.cursor)
            .map_err(|e| JobError::fatal(format!("encoding a cursor: {}", redact::json(&e))))?,
        token: page.cursor.token(),
        read_up_to: None,
        changes: page.changes,
        index: page.index,
        labels: None,
        zoho_labels: None,
        imap: page.locations,
        idle: None,
        checking: false,
        phase: page.phase,
        more: page.more,
    })
}

/// When the stream's first sync completed, as its `cursor` keeps it
/// (`synced_at`), set to `now` once the stream is past it (`phase`, after
/// the page): a stream connected before this was kept counts from its next
/// run.
fn synced_at(
    cursor: &mut serde_json::Value,
    phase: Phase,
    now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    if let Some(at) = cursor
        .get("synced_at")
        .and_then(|at| serde_json::from_value::<DateTime<Utc>>(at.clone()).ok())
    {
        return Some(at);
    }
    if phase == Phase::Backfill || !cursor.is_object() {
        return None;
    }
    cursor["synced_at"] = serde_json::json!(now);
    Some(now)
}

/// `changes` for `inbox` as batches: the messages the stream's first sync
/// read (all of them, `first_sync`), and those it indexes now that were
/// received before that sync completed (`synced`; all, when it has not),
/// in a `backfill` batch first; the rest in another.
fn split(
    inbox: InboxId,
    changes: Vec<MessageChange>,
    index: &[IndexOp],
    first_sync: bool,
    synced: Option<DateTime<Utc>>,
) -> Vec<MessageChangeBatch> {
    let old: std::collections::HashSet<&str> = index
        .iter()
        .filter_map(|op| match op {
            IndexOp::Insert {
                provider_message_id,
                internal_date,
                ..
            } if first_sync || synced.is_none_or(|at| *internal_date < at) => {
                Some(provider_message_id.as_str())
            }
            _ => None,
        })
        .collect();
    let (backfill, rest): (Vec<MessageChange>, Vec<MessageChange>) =
        changes.into_iter().partition(|change| match change {
            MessageChange::Upsert { message } => {
                first_sync || old.contains(message.provider_message_id.as_str())
            }
            MessageChange::Remove { .. } => first_sync,
        });
    let mut batches = Vec::new();
    if !backfill.is_empty() {
        let mut batch = MessageChangeBatch::new(inbox, backfill);
        batch.backfill = true;
        batches.push(batch);
    }
    if !rest.is_empty() {
        batches.push(MessageChangeBatch::new(inbox, rest));
    }
    batches
}

/// Replace the cached Gmail label names of `stream` with `labels`.
async fn replace_labels(
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    stream: StreamId,
    labels: &[LabelInfo],
) -> Result<(), JobError> {
    sqlx::query!(
        "DELETE FROM connectors.gmail_labels WHERE stream_id = $1",
        stream as StreamId
    )
    .execute(&mut **tx)
    .await?;
    for label in labels {
        sqlx::query!(
            r#"INSERT INTO connectors.gmail_labels (workspace_id, stream_id, label_id, name, kind)
                       VALUES ($1, $2, $3, $4, $5)"#,
            workspace as WorkspaceId,
            stream as StreamId,
            label.id,
            label.name,
            label.kind.as_deref().unwrap_or("system"),
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// Replace the cached Zoho label names of `stream` with `labels`.
async fn replace_zoho_labels(
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    stream: StreamId,
    labels: &[zoho::ZohoLabel],
    now: DateTime<Utc>,
) -> Result<(), JobError> {
    sqlx::query("DELETE FROM connectors.zoho_labels WHERE stream_id = $1")
        .bind(stream)
        .execute(&mut **tx)
        .await?;
    for label in labels {
        sqlx::query(
            "INSERT INTO connectors.zoho_labels
                 (workspace_id, stream_id, label_id, name, refreshed_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (stream_id, label_id) DO UPDATE SET name = EXCLUDED.name",
        )
        .bind(workspace)
        .bind(stream)
        .bind(&label.label_id)
        .bind(label.name())
        .bind(now)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// The cached labels of `stream`'s provider (id, name): Gmail's user labels,
/// or the Zoho account's labels.
async fn label_rows(
    ctx: &JobContext,
    workspace: WorkspaceId,
    stream: StreamId,
    provider: Provider,
) -> Result<Vec<(String, String)>, JobError> {
    let query = match provider {
        Provider::Google => {
            "SELECT label_id, name FROM connectors.gmail_labels
              WHERE stream_id = $1 AND kind = 'user'"
        }
        Provider::Zoho => "SELECT label_id, name FROM connectors.zoho_labels WHERE stream_id = $1",
        Provider::Imap => return Ok(Vec::new()),
    };
    let mut tx = ctx.pool().begin_for(workspace).await?;
    let rows: Vec<(String, String)> = sqlx::query_as(query)
        .bind(stream)
        .fetch_all(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(rows)
}

/// The cached names of `stream`'s Gmail user labels (id → name), as the
/// run's read left them.
async fn gmail_names(
    ctx: &JobContext,
    workspace: WorkspaceId,
    stream: StreamId,
) -> Result<HashMap<String, String>, JobError> {
    Ok(label_rows(ctx, workspace, stream, Provider::Google)
        .await?
        .into_iter()
        .collect())
}

/// The names of `labels` (id, name) and of the marks `pushed` created.
fn named(labels: &[(String, String)], pushed: &Push) -> HashMap<String, String> {
    labels.iter().chain(&pushed.created).cloned().collect()
}

/// The messages whose provider stream (`stream_of` their labels) the
/// writes of `pushed` changed.
fn stream_changed(
    backlog: &Backlog,
    pushed: &Push,
    stream_of: impl Fn(&[String]) -> Option<Option<ProviderStream>>,
) -> Vec<String> {
    pushed
        .outcome
        .index
        .iter()
        .filter(|(message, after)| {
            let before = backlog
                .labels
                .get(message)
                .map(Vec::as_slice)
                .unwrap_or_default();
            stream_of(before) != stream_of(after)
        })
        .map(|(message, _)| message.clone())
        .collect()
}

/// The messages a run reports ([`report`]), and how the provider names
/// the stream marks of their pending changes.
struct Reporting<'a> {
    /// The messages.
    ids: &'a [String],
    /// The stream marks the provider has.
    marks: MarkIds,
    /// A label's token from its id.
    label: fn(&str) -> String,
}

/// Report once, from the index, where the messages of `reporting` are now
/// that a write changed their provider stream (no download, no provider
/// call), so the conversations module stores it; a message the page carries
/// already is not repeated. A member's change still waiting to be made
/// shows as they left it, as in every emission. `flags` reads a message's
/// state from its labels. A message whose stored document cannot be read
/// is left out (logged).
async fn report(
    ctx: &JobContext,
    workspace: WorkspaceId,
    stream: StreamId,
    reporting: &Reporting<'_>,
    changes: &mut Vec<MessageChange>,
    flags: impl Fn(&[String]) -> crate::mail::Flags,
) -> Result<(), JobError> {
    let carried: std::collections::HashSet<&str> = changes
        .iter()
        .filter_map(|change| match change {
            MessageChange::Upsert { message } => Some(message.provider_message_id.as_str()),
            MessageChange::Remove { .. } => None,
        })
        .collect();
    let ids: Vec<&str> = reporting
        .ids
        .iter()
        .map(String::as_str)
        .filter(|id| !carried.contains(id))
        .collect();
    if ids.is_empty() {
        return Ok(());
    }
    let mut tx = ctx.pool().begin_for(workspace).await?;
    let rows: Vec<(String, Vec<String>, String, i64)> = sqlx::query_as(
        "SELECT provider_message_id, label_ids, document_hash, document_size
           FROM connectors.messages
          WHERE stream_id = $1 AND provider_message_id = ANY($2)
          ORDER BY internal_date",
    )
    .bind(stream)
    .bind(&ids)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let pending: Vec<String> = rows.iter().map(|(id, ..)| id.clone()).collect();
    let overlay = outbound::overlay(ctx.pool(), workspace, stream, &pending).await?;
    let key = workspace.to_string();
    let blobs = &ctx.state().blobs;
    for (id, labels, hash, size) in rows {
        let labels = overlay.get(&id).map_or_else(
            || labels.clone(),
            |changes| {
                let changes = crate::domain::marks::resolve_changes(
                    changes,
                    &reporting.marks,
                    reporting.label,
                );
                outbound::apply_labels(&labels, &changes)
            },
        );
        let document = match crate::mail::document_ref(&hash, size) {
            Ok(blob) => crate::mail::document(blobs, &key, &blob).await,
            Err(err) => Err(err),
        };
        let mut message = match document {
            Ok(message) => message,
            Err(err) => {
                tracing::warn!(%workspace, %stream, error = %err, "a stream write could not be reported");
                continue;
            }
        };
        crate::mail::apply_flags(&mut message, &flags(&labels));
        changes.push(MessageChange::Upsert {
            message: Box::new(message),
        });
    }
    Ok(())
}

/// The end of a run's outbound work, in its transaction (after the stream
/// row is locked by the run's update, so a change recorded concurrently is
/// either seen here or enqueues its own run): prune old changes and close
/// open write windows at the history id the run read up to. Returns whether changes arrived
/// during this run (another run is needed now).
async fn settle_outbound(
    tx: &mut Transaction<'static, Postgres>,
    stream: StreamId,
    pushed_up_to: Option<uuid::Uuid>,
    before: DateTime<Utc>,
    read_up_to: Option<i64>,
) -> Result<bool, JobError> {
    outbound::prune(tx, stream, before, read_up_to).await?;
    Ok(outbound::more_pending(tx, stream, pushed_up_to).await?)
}

impl Target {
    /// Where this run makes the stream's changes: the connection and the
    /// mailbox loaded with it.
    fn at(&self) -> outbound::At<'_> {
        outbound::At {
            connection: self.connection,
            mailbox: &self.mailbox_key,
        }
    }
}

/// Whether `stream` reads a calendar (a stream of the `calendar`
/// capability), read in a short transaction of its own.
async fn is_calendar(
    ctx: &JobContext,
    workspace: WorkspaceId,
    stream: StreamId,
) -> Result<bool, JobError> {
    let mut tx = ctx.pool().begin_for(workspace).await?;
    let capability: Option<String> =
        sqlx::query_scalar("SELECT capability FROM connectors.streams WHERE id = $1")
            .bind(stream)
            .fetch_optional(&mut *tx)
            .await?;
    tx.commit().await?;
    Ok(capability.as_deref() == Some("calendar"))
}

/// The stream and its connection, read in a short transaction of its own.
async fn load(
    ctx: &JobContext,
    workspace: WorkspaceId,
    stream: StreamId,
) -> Result<Option<Target>, JobError> {
    let mut tx = ctx.pool().begin_for(workspace).await?;
    let row = sqlx::query!(
        r#"SELECT s.id AS "stream: StreamId", s.connection_id AS "connection: ConnectionId",
                  c.provider, c.provider_server, c.granted_by AS "granted_by: UserId", c.health,
                  s.mailbox_key, s.inbox_id AS "inbox: InboxId", s.phase, s.cursor,
                  s.interval_seconds,
                  s.paused, s.consecutive_failures, s.throttled_until
             FROM connectors.streams s
             JOIN connectors.connections c ON c.id = s.connection_id
            WHERE s.id = $1"#,
        stream as StreamId,
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(row) = row else {
        tx.commit().await?;
        return Ok(None);
    };
    let scopes: String =
        sqlx::query_scalar("SELECT scopes FROM connectors.connections WHERE id = $1")
            .bind(row.connection)
            .fetch_one(&mut *tx)
            .await?;
    tx.commit().await?;
    Ok(Some(Target {
        labels_granted: zoho::labels_granted(&scopes),
        stream: row.stream,
        connection: row.connection,
        provider: Provider::parse(&row.provider)?,
        provider_server: row.provider_server,
        mailbox_key: row.mailbox_key,
        granted_by: row.granted_by,
        health: Health::parse(&row.health)?,
        inbox: row.inbox,
        phase: Phase::parse(&row.phase)?,
        cursor: row.cursor,
        interval_seconds: row.interval_seconds,
        paused: row.paused,
        consecutive_failures: row.consecutive_failures,
        throttled_until: row.throttled_until,
    }))
}

/// Release the batches of `stream` older than `before`.
async fn prune_batches(
    tx: &mut Transaction<'static, Postgres>,
    state: &campbooks_kernel::AppState,
    workspace: WorkspaceId,
    stream: StreamId,
    before: DateTime<Utc>,
) -> Result<(), JobError> {
    let old = sqlx::query_scalar!(
        "DELETE FROM connectors.batches WHERE stream_id = $1 AND created_at < $2 RETURNING id",
        stream as StreamId,
        before,
    )
    .fetch_all(&mut **tx)
    .await?;
    let key = workspace.to_string();
    for id in old {
        state
            .blobs
            .detach_owner(tx, &key, &BlobOwner::new(BATCH_OWNER, id))
            .await?;
    }
    Ok(())
}

/// Read one page of `stream` without pushing or applying it: the changes
/// it would emit, as JSON (tests of the reader's overlay).
#[cfg(test)]
pub(crate) async fn read_page(
    state: &campbooks_kernel::AppState,
    deps: &Deps,
    workspace: WorkspaceId,
    stream: StreamId,
) -> Vec<serde_json::Value> {
    let mut tx = state.db.begin_for(workspace).await.unwrap();
    let row = sqlx::query!(
        r#"SELECT connection_id AS "connection: ConnectionId", phase, cursor
             FROM connectors.streams WHERE id = $1"#,
        stream as StreamId
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let token = credentials::access_token(&state.db, deps, workspace, row.connection)
        .await
        .unwrap_or_else(|_| panic!("a token"));
    let google = deps.google.clone().unwrap();
    let reader = Reader {
        gmail: google.gmail(&token),
        pool: &state.db,
        blobs: &state.blobs,
        workspace,
        stream,
        phase: Phase::parse(&row.phase).unwrap(),
        cursor: serde_json::from_value(row.cursor).unwrap(),
        now: deps.clock.now(),
        backfill_days: deps.config.gmail.backfill_days,
        max_message_bytes: deps.config.gmail.max_message_bytes,
        run_budget_bytes: deps.config.gmail.run_budget_bytes,
    };
    let page = reader.read().await.unwrap();
    page.changes
        .iter()
        .map(|c| serde_json::to_value(c).unwrap())
        .collect()
}

#[cfg(test)]
mod calendar_tests;
#[cfg(test)]
mod imap_tests;
#[cfg(test)]
mod imap_write_tests;
#[cfg(test)]
mod tests;
