//! A calendar stream's run: make the changes members asked for, then read
//! the syncing calendars from their sync tokens and announce what changed
//! (`connectors.calendar_changed`), moving each calendar's token in the
//! transaction that publishes its batch, so a batch is never lost or applied
//! twice.
//!
//! Like a mail run, it takes its connection's budget, follows the run's
//! backoff and throttling, and records each change's end as it happens (a
//! write that happened is never forgotten because the read after it
//! failed). Unlike a mail run, trouble here marks the *stream* (a reason a
//! member can act on, such as the calendar API not being enabled): the
//! connection's health says nothing of one capability, and mail goes on.
//!
//! Errors are built from static words only: they are kept with the job.

use std::collections::HashMap;

use campbooks_kernel::{
    AppState, JobContext, JobError,
    blobs::{BlobOwner, BlobRef},
    db::BeginFor as _,
    jobs::EnqueueOptions,
    tenancy::WorkspaceId,
};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::{SyncStream, Target, prune_batches};
use crate::{
    calendar::{
        self, API_NOT_ENABLED, BATCH_CONTENT_TYPE, BatchEvent, ChangeDocument, KnownCalendar,
        LIST_CONTENT_TYPE, ListDocument, Source, WriteFailure, Written,
        changes::{self, MAX_ATTEMPTS, Recorded, State},
    },
    domain::{
        credentials::{self, TokenError},
        health,
        model::{Capability, ConnectionId, Phase, Provider, StreamId},
        removal::{BATCH_OWNER, CONNECTION_OWNER},
        schedule::{self, LEASE, Rate, SYNC_STREAM, SyncStreamPayload},
        tracked,
    },
    events::{
        ACCOUNT_CONTENT_TYPE, AccountDocument, CalendarChangeCompleted, CalendarChanged,
        CalendarOutcome, CalendarReason, CapabilityGranted,
    },
    google::CalendarError,
    metrics::{self, SyncOutcome},
    provider::ProviderError,
    tracking,
};

/// What a run's push did.
#[derive(Debug, Default)]
struct Pushed {
    /// Something was written at the provider.
    wrote: bool,
    /// The provider's trouble, which stopped the push.
    trouble: Option<ProviderError>,
    /// The calendar API is not enabled: the push and the read stop.
    not_enabled: bool,
}

/// What a run's reading of one calendar gave, to apply.
#[derive(Debug)]
struct Done {
    /// The calendar's row.
    calendar: Uuid,
    /// The token to read from next.
    token: Option<String>,
    /// It was read whole.
    full: bool,
    /// Its batches, stored, with how many changes each holds.
    batches: Vec<(BlobRef, usize)>,
}

/// Why a run stopped reading.
#[derive(Debug)]
enum Stop {
    /// The provider's trouble.
    Trouble(ProviderError),
    /// The calendar API is not enabled.
    NotEnabled,
}

impl SyncStream {
    /// One run of a calendar stream. `Ok(())` once it has ended (waiting,
    /// trouble recorded, or applied).
    #[allow(
        clippy::too_many_lines,
        reason = "one run read top to bottom: token, backlog, push, read, apply"
    )]
    pub(super) async fn run_calendar(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let google = self.deps.google.clone();
        let zoho = self.deps.zoho.clone();
        let centre = zoho
            .as_deref()
            .zip(target.provider_server.as_deref())
            .and_then(|(zoho, server)| zoho.centre(server));
        let offered = match target.provider {
            Provider::Google => google.is_some(),
            Provider::Zoho => zoho.is_some() && self.deps.config.zoho_calendar,
            Provider::Imap | Provider::Microsoft => false,
        };
        if !offered {
            // The provider is no longer configured: the leased schedule row
            // brings the stream back later.
            return Ok(());
        }
        let token =
            match credentials::access_token(ctx.pool(), &self.deps, workspace, target.connection)
                .await
            {
                Ok(token) => token,
                Err(TokenError::NeedsReauth(reason)) => {
                    metrics::sync_finished(target.provider, SyncOutcome::Refused);
                    return self.needs_reauth(ctx, workspace, target, reason, now).await;
                }
                Err(TokenError::Provider(err)) => {
                    return self.trouble(ctx, workspace, target, &err, now).await;
                }
                Err(TokenError::Database(err)) => return Err(err.into()),
            };
        let source = match (
            target.provider,
            google.as_deref(),
            zoho.as_deref(),
            centre.as_ref(),
        ) {
            (Provider::Google, Some(google), _, _) => Source::Google(google.calendar(&token)),
            (Provider::Zoho, _, Some(zoho), Some(centre)) => {
                Source::Zoho(zoho.calendar(centre, &token))
            }
            _ => {
                self.needs_reauth(ctx, workspace, target, "unknown_data_centre", now)
                    .await?;
                return Ok(());
            }
        };

        // Changes given up, refused, and the ones this run makes.
        let state = ctx.state().clone();
        let retention = TimeDelta::days(i64::from(self.deps.config.batch_retention_days));
        let (backlog, known) = {
            let mut tx = ctx.pool().begin_for(workspace).await?;
            let expired = changes::expire(&mut tx, target.stream, now - retention, now).await?;
            let backlog = changes::backlog(
                &mut tx,
                &state.binding,
                workspace,
                (target.stream, target.connection),
                now,
            )
            .await?;
            for row in &expired {
                announce(
                    &state,
                    &mut tx,
                    workspace,
                    row,
                    (
                        CalendarOutcome::Failed,
                        Some(CalendarReason::Unavailable),
                        None,
                    ),
                )
                .await?;
            }
            for row in &backlog.refused {
                announce(
                    &state,
                    &mut tx,
                    workspace,
                    row,
                    (
                        CalendarOutcome::Skipped,
                        Some(CalendarReason::Unverified),
                        None,
                    ),
                )
                .await?;
            }
            let known = calendar::known(&mut tx, target.stream).await?;
            tx.commit().await?;
            (backlog, known)
        };

        let result = if known.is_empty() {
            self.list_calendars(ctx, workspace, target, &source, now)
                .await
        } else {
            self.sync_calendars(ctx, workspace, target, &source, &known, &backlog, now)
                .await
        };
        result.inspect_err(|_| metrics::sync_finished(target.provider, SyncOutcome::Failed))
    }

    /// A stream that knows no calendar lists the account's (a grant that
    /// could not, or a stream made before they could be listed) and announces
    /// them for the schedule module to make its calendars.
    async fn list_calendars(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        source: &Source<'_>,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let rate = Rate::per_minute(self.deps.config.calendar.requests_per_minute);
        let key = schedule::calendar_budget(target.connection);
        if let Err(wait) = schedule::take(ctx.pool(), &key, 1, rate).await? {
            metrics::sync_finished(target.provider, SyncOutcome::Throttled);
            Self::wait_for_budget(ctx, workspace, target.stream, wait, now).await?;
            return Ok(());
        }
        let listed = match source.list().await {
            Ok(listed) => listed,
            Err(CalendarError::ApiNotEnabled) => {
                return self.calendar_disabled(ctx, workspace, target, now).await;
            }
            Err(CalendarError::Provider(err)) => {
                return self
                    .calendar_trouble(ctx, workspace, target, &err, now)
                    .await;
            }
            Err(_) => {
                let err = ProviderError::Unexpected("the calendar list was refused".into());
                return self
                    .calendar_trouble(ctx, workspace, target, &err, now)
                    .await;
            }
        };
        if listed.is_empty() {
            // An account with no calendar to announce: look again at the
            // slowest interval.
            metrics::sync_finished(target.provider, SyncOutcome::Ok);
            let wait = TimeDelta::seconds(i64::from(self.deps.config.poll.max_seconds));
            schedule::set_due(ctx.tx().await?, workspace, target.stream, now + wait).await?;
            return Ok(());
        }
        let state = ctx.state().clone();
        let key = workspace.to_string();
        let document = ListDocument {
            version: 1,
            provider: match target.provider {
                Provider::Google => Some("google"),
                Provider::Zoho => Some("zoho"),
                Provider::Imap | Provider::Microsoft => None,
            },
            calendars: listed.clone(),
        };
        let list = state
            .blobs
            .put(
                &key,
                serde_json::to_vec(&document)
                    .map_err(|_| JobError::fatal("a calendar list did not encode"))?,
                LIST_CONTENT_TYPE,
            )
            .await?;
        let account = account_blob(&state, workspace, target.connection, target.provider).await?;
        let tx = ctx.tx().await?;
        health::lock(tx, target.connection).await?;
        calendar::remember(tx, workspace, target.stream, &listed).await?;
        state
            .blobs
            .attach(
                tx,
                &key,
                &list,
                &BlobOwner::new(CONNECTION_OWNER, uuid::Uuid::from(target.connection)),
            )
            .await?;
        state
            .events
            .publish(
                tx,
                &CapabilityGranted {
                    workspace: key,
                    connection: target.connection,
                    capability: Capability::Calendar,
                    granted_by: target.granted_by,
                    inbox_id: None,
                    account,
                    calendars: Some(list),
                },
            )
            .await?;
        sqlx::query(
            "UPDATE connectors.streams
                SET health_reason = NULL, consecutive_failures = 0, throttled_until = NULL
              WHERE id = $1",
        )
        .bind(target.stream)
        .execute(&mut **tx)
        .await?;
        // The calendars are known now: read the one that syncs.
        schedule::set_due(tx, workspace, target.stream, now).await?;
        metrics::sync_finished(target.provider, SyncOutcome::Ok);
        tracing::info!(%workspace, stream = %target.stream, calendars = listed.len(), "calendars listed");
        Ok(())
    }

    /// Push the stream's pending changes, then read its syncing calendars
    /// and apply what was read.
    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "one run read top to bottom: budget, push, read, apply"
    )]
    async fn sync_calendars(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        source: &Source<'_>,
        known: &[KnownCalendar],
        backlog: &changes::Backlog,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let syncing: Vec<&KnownCalendar> = known.iter().filter(|c| c.syncing).collect();
        let cost =
            u32::try_from(1 + backlog.pending.len() * 3 + syncing.len() * 2).unwrap_or(u32::MAX);
        let rate = Rate::per_minute(self.deps.config.calendar.requests_per_minute);
        let budget = schedule::calendar_budget(target.connection);
        if let Err(wait) = schedule::take(ctx.pool(), &budget, cost, rate).await? {
            metrics::sync_finished(target.provider, SyncOutcome::Throttled);
            Self::wait_for_budget(ctx, workspace, target.stream, wait, now).await?;
            return Ok(());
        }

        let pushed = self
            .push_calendar(ctx, workspace, source, known, backlog, now)
            .await?;
        if pushed.not_enabled {
            return self.calendar_disabled(ctx, workspace, target, now).await;
        }
        if let Some(err) = &pushed.trouble {
            return self
                .calendar_trouble(ctx, workspace, target, err, now)
                .await;
        }

        let state = ctx.state().clone();
        let key = workspace.to_string();
        let retention = TimeDelta::days(i64::from(self.deps.config.batch_retention_days));
        let echoes = {
            let mut tx = ctx.pool().begin_for(workspace).await?;
            let echoes = changes::echoes(&mut tx, target.stream, now - retention).await?;
            tx.commit().await?;
            echoes
        };
        let mut done: Vec<Done> = Vec::new();
        let mut stop: Option<Stop> = None;
        for calendar in &syncing {
            let id = calendar.provider_calendar_id.as_str();
            let mut answer = source.read(id, calendar.sync_token.as_deref(), now).await;
            if matches!(answer, Err(CalendarError::Gone)) {
                // The token is no longer accepted: read the calendar whole.
                answer = source.read(id, None, now).await;
            }
            match answer {
                Ok(mut read) => {
                    read.upserts.retain(|event| !echoed(&echoes, event));
                    let (token, full) = (read.token.clone(), read.full);
                    let mut batches = Vec::new();
                    for batch in read.batches(id) {
                        let count = batch.upserts.len() + batch.removals.len();
                        let bytes = serde_json::to_vec(&batch)
                            .map_err(|_| JobError::fatal("a calendar batch did not encode"))?;
                        let blob = state.blobs.put(&key, bytes, BATCH_CONTENT_TYPE).await?;
                        batches.push((blob, count));
                    }
                    done.push(Done {
                        calendar: calendar.id,
                        token,
                        full,
                        batches,
                    });
                }
                Err(CalendarError::ApiNotEnabled) => {
                    stop = Some(Stop::NotEnabled);
                    break;
                }
                Err(CalendarError::Provider(err)) => {
                    stop = Some(Stop::Trouble(err));
                    break;
                }
                Err(_) => {
                    stop = Some(Stop::Trouble(ProviderError::Unexpected(
                        "a calendar read was refused".into(),
                    )));
                    break;
                }
            }
        }

        // What was read is applied whatever came after it.
        let changed = pushed.wrote || done.iter().any(|d| !d.batches.is_empty());
        let batches: usize = done.iter().map(|d| d.batches.len()).sum();
        let interval =
            schedule::next_interval(target.interval_seconds, changed, &self.deps.config.poll);
        let tx = ctx.tx().await?;
        // The connection before the stream (removal's order).
        health::lock(tx, target.connection).await?;
        for read in &done {
            for (blob, count) in &read.batches {
                store_batch(
                    tx,
                    &state,
                    (workspace, target.stream, target.connection),
                    read.calendar,
                    blob,
                    *count,
                    now,
                )
                .await?;
            }
            calendar::read_up_to(tx, read.calendar, read.token.as_deref(), read.full, now).await?;
        }
        changes::prune(tx, target.stream, now - retention).await?;
        prune_batches(tx, &state, workspace, target.stream, now - retention).await?;
        let more = changes::more_pending(tx, target.stream, backlog.last()).await?;
        sqlx::query(
            "UPDATE connectors.streams
                SET phase = 'incremental', interval_seconds = $2, last_synced_at = $3,
                    last_change_at = CASE WHEN $4 THEN $3 ELSE last_change_at END,
                    consecutive_failures = 0, throttled_until = NULL,
                    health_reason = CASE WHEN $5 THEN health_reason ELSE NULL END
              WHERE id = $1",
        )
        .bind(target.stream)
        .bind(interval)
        .bind(now)
        .bind(changed)
        .bind(stop.is_some())
        .execute(&mut **tx)
        .await?;
        // One summary per run, only when it stored something or moved on.
        if batches > 0 || target.phase != Phase::Incremental {
            let stored: usize = done
                .iter()
                .flat_map(|read| read.batches.iter().map(|(_, count)| *count))
                .sum();
            let (stream, provider) = tracked::stream(tx, target.stream).await?;
            state
                .tracker
                .track(
                    tx,
                    &tracking::StreamSynced {
                        workspace,
                        stream,
                        provider,
                        before_phase: tracked::phase(target.phase),
                        changes: i64::try_from(stored).unwrap_or(i64::MAX),
                        batches: i64::try_from(batches).unwrap_or(i64::MAX),
                    },
                )
                .await?;
        }
        if let Some(stop) = stop {
            // What was read is applied above; the trouble sets the next run.
            return match stop {
                Stop::NotEnabled => self.calendar_disabled(ctx, workspace, target, now).await,
                Stop::Trouble(err) => {
                    self.calendar_trouble(ctx, workspace, target, &err, now)
                        .await
                }
            };
        }
        if more {
            // Changes arrived during the run: run again straight away,
            // holding the schedule row so the dispatcher does not double it.
            schedule::set_due(tx, workspace, target.stream, now + LEASE).await?;
            campbooks_kernel::jobs::enqueue_to::<Self>(
                tx,
                &SyncStreamPayload {
                    stream: target.stream,
                },
                EnqueueOptions::for_workspace(key)
                    .ordered_by(format!("{SYNC_STREAM}:{}", target.stream)),
            )
            .await?;
        } else {
            let next = now + TimeDelta::seconds(i64::from(interval));
            schedule::set_due(tx, workspace, target.stream, next).await?;
        }
        metrics::sync_finished(target.provider, SyncOutcome::Ok);
        tracing::info!(
            %workspace, stream = %target.stream, calendars = done.len(), batches, more,
            "calendar stream synced"
        );
        Ok(())
    }

    /// Make the pending changes of the backlog, one at a time, recording
    /// how each ended before the next is made. A change that met provider
    /// trouble stays pending (until it has met it [`MAX_ATTEMPTS`] times) and
    /// stops the push.
    async fn push_calendar(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        source: &Source<'_>,
        known: &[KnownCalendar],
        backlog: &changes::Backlog,
        now: DateTime<Utc>,
    ) -> Result<Pushed, JobError> {
        let mut pushed = Pushed::default();
        let state = ctx.state().clone();
        for (n, row) in backlog.pending.iter().enumerate() {
            let outcome = self.make_change(ctx, workspace, source, known, row).await?;
            let mut tx = ctx.pool().begin_for(workspace).await?;
            match outcome {
                Made::Ended(state_of, outcome, reason, written) => {
                    changes::end(
                        &mut tx,
                        row.id,
                        state_of,
                        reason.map(reason_code),
                        written.as_ref(),
                        now,
                    )
                    .await?;
                    announce(
                        &state,
                        &mut tx,
                        workspace,
                        row,
                        (outcome, reason, written.as_ref()),
                    )
                    .await?;
                    pushed.wrote |= state_of == State::Applied;
                }
                Made::Trouble(err) => {
                    let attempts = changes::attempted(&mut tx, row.id).await?;
                    if attempts >= MAX_ATTEMPTS {
                        changes::end(
                            &mut tx,
                            row.id,
                            State::Failed,
                            Some(reason_code(CalendarReason::TooManyAttempts)),
                            None,
                            now,
                        )
                        .await?;
                        announce(
                            &state,
                            &mut tx,
                            workspace,
                            row,
                            (
                                CalendarOutcome::Failed,
                                Some(CalendarReason::TooManyAttempts),
                                None,
                            ),
                        )
                        .await?;
                    }
                    pushed.trouble = Some(err);
                }
                Made::NotEnabled => {
                    // This and every change after it: nothing can be made.
                    for rest in &backlog.pending[n..] {
                        changes::end(
                            &mut tx,
                            rest.id,
                            State::Failed,
                            Some(reason_code(CalendarReason::ApiNotEnabled)),
                            None,
                            now,
                        )
                        .await?;
                        announce(
                            &state,
                            &mut tx,
                            workspace,
                            rest,
                            (
                                CalendarOutcome::Failed,
                                Some(CalendarReason::ApiNotEnabled),
                                None,
                            ),
                        )
                        .await?;
                    }
                    pushed.not_enabled = true;
                }
            }
            tx.commit().await?;
            if pushed.trouble.is_some() || pushed.not_enabled {
                break;
            }
        }
        Ok(pushed)
    }

    /// Make one change, or say why not. The document is the one the
    /// schedule module stored (the row's blob), and the calendar it names
    /// must be one this stream lists.
    async fn make_change(
        &self,
        ctx: &JobContext,
        workspace: WorkspaceId,
        source: &Source<'_>,
        known: &[KnownCalendar],
        row: &Recorded,
    ) -> Result<Made, JobError> {
        let skipped =
            |reason| Made::Ended(State::Skipped, CalendarOutcome::Skipped, Some(reason), None);
        let Some(document) = changes::read_json::<ChangeDocument>(
            &ctx.state().blobs,
            workspace,
            &row.body.0,
            changes::MAX_CHANGE_BYTES,
        )
        .await?
        else {
            return Ok(skipped(CalendarReason::Rejected));
        };
        let named = known
            .iter()
            .any(|c| c.provider_calendar_id == document.provider_calendar_id);
        if !document.is_sound() || document.change != row.change || !named {
            return Ok(skipped(CalendarReason::Rejected));
        }
        Ok(match source.write(&document, row.request_id).await {
            Ok(written) => Made::Ended(
                State::Applied,
                CalendarOutcome::Applied,
                None,
                Some(written),
            ),
            Err(WriteFailure::Superseded) => Made::Ended(
                State::Superseded,
                CalendarOutcome::Superseded,
                Some(CalendarReason::ChangedAtProvider),
                None,
            ),
            Err(WriteFailure::Skip(reason)) => skipped(match reason {
                "not_found" => CalendarReason::NotFound,
                _ => CalendarReason::Rejected,
            }),
            Err(WriteFailure::Trouble(err)) => Made::Trouble(err),
            Err(WriteFailure::ApiNotEnabled) => Made::NotEnabled,
        })
    }

    /// The calendar API is not enabled for the app: the stream says so for
    /// the member (or the operator) to act on, and tries again at the
    /// slowest interval. Mail is untouched.
    async fn calendar_disabled(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        metrics::sync_finished(target.provider, SyncOutcome::Failed);
        let tx = ctx.tx().await?;
        sqlx::query(
            "UPDATE connectors.streams SET health_reason = $2, consecutive_failures = 0
              WHERE id = $1",
        )
        .bind(target.stream)
        .bind(API_NOT_ENABLED)
        .execute(&mut **tx)
        .await?;
        let wait = TimeDelta::seconds(i64::from(self.deps.config.poll.max_seconds));
        schedule::set_due(tx, workspace, target.stream, now + wait).await?;
        tracing::warn!(%workspace, stream = %target.stream, "the calendar API is not enabled for the app");
        Ok(())
    }

    /// The provider failed on a calendar stream. A refused token or grant is
    /// the connection's (shared with mail); anything else marks only this
    /// stream: throttling holds it back, and failures in a row give it a
    /// reason.
    async fn calendar_trouble(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        err: &ProviderError,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let poll = &self.deps.config.poll;
        match err {
            ProviderError::Unauthorized | ProviderError::InvalidGrant => {
                self.trouble(ctx, workspace, target, err, now).await
            }
            ProviderError::RateLimited { retry_after } => {
                metrics::sync_finished(target.provider, SyncOutcome::Throttled);
                let wait = retry_after
                    .and_then(|d| TimeDelta::from_std(d).ok())
                    .unwrap_or(TimeDelta::seconds(i64::from(poll.min_seconds)));
                let until = now + wait;
                let tx = ctx.tx().await?;
                sqlx::query("UPDATE connectors.streams SET throttled_until = $2 WHERE id = $1")
                    .bind(target.stream)
                    .bind(until)
                    .execute(&mut **tx)
                    .await?;
                schedule::set_due(tx, workspace, target.stream, until).await?;
                Ok(())
            }
            ProviderError::NotFound
            | ProviderError::Unavailable(_)
            | ProviderError::Unexpected(_) => {
                metrics::sync_finished(target.provider, SyncOutcome::Failed);
                let failures = target.consecutive_failures.saturating_add(1);
                let max = i64::from(poll.max_seconds);
                let backoff = i64::from(poll.min_seconds)
                    .saturating_mul(1_i64 << failures.clamp(0, 16))
                    .min(max);
                let failing =
                    u32::try_from(failures).unwrap_or(u32::MAX) >= self.deps.config.failing_after;
                let tx = ctx.tx().await?;
                sqlx::query(
                    "UPDATE connectors.streams
                        SET consecutive_failures = $2,
                            health_reason = CASE WHEN $3 THEN 'provider_errors'
                                                 ELSE health_reason END
                      WHERE id = $1",
                )
                .bind(target.stream)
                .bind(failures)
                .bind(failing)
                .execute(&mut **tx)
                .await?;
                schedule::set_due(
                    tx,
                    workspace,
                    target.stream,
                    now + TimeDelta::seconds(backoff),
                )
                .await?;
                tracing::warn!(%workspace, stream = %target.stream, failures, "a calendar run failed");
                Ok(())
            }
        }
    }
}

/// The account document of `connection`, as a blob (the same bytes as the
/// one its grant stored).
async fn account_blob(
    state: &AppState,
    workspace: WorkspaceId,
    connection: ConnectionId,
    provider: Provider,
) -> Result<BlobRef, JobError> {
    let mut tx = state.db.begin_for(workspace).await?;
    let row: (String, bool, Option<String>) = sqlx::query_as(
        "SELECT address, address_verified, display_name
           FROM connectors.connections WHERE id = $1",
    )
    .bind(connection)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    let document = AccountDocument {
        version: 1,
        provider,
        address: row.0,
        address_verified: row.1,
        display_name: row.2,
    };
    Ok(state
        .blobs
        .put(
            &workspace.to_string(),
            serde_json::to_vec(&document)
                .map_err(|_| JobError::fatal("an account document did not encode"))?,
            ACCOUNT_CONTENT_TYPE,
        )
        .await?)
}

/// How one change ended in a run.
#[derive(Debug)]
enum Made {
    /// It ended: its state, the outcome announced, why, what was answered.
    Ended(
        State,
        CalendarOutcome,
        Option<CalendarReason>,
        Option<Written>,
    ),
    /// The provider's trouble: it stays pending.
    Trouble(ProviderError),
    /// The calendar API is not enabled.
    NotEnabled,
}

/// The stored code of a reason (the contract's name for it).
const fn reason_code(reason: CalendarReason) -> &'static str {
    match reason {
        CalendarReason::ChangedAtProvider => "changed_at_provider",
        CalendarReason::Unverified => "unverified",
        CalendarReason::NotConnected => "not_connected",
        CalendarReason::NeedsReauth => "needs_reauth",
        CalendarReason::Rejected => "rejected",
        CalendarReason::NotFound => "not_found",
        CalendarReason::Unavailable => "unavailable",
        CalendarReason::ApiNotEnabled => "api_not_enabled",
        CalendarReason::TooManyAttempts => "too_many_attempts",
    }
}

/// Whether the provider reports `event` only because this module wrote it
/// (the etag it answered): not a change.
fn echoed(echoes: &HashMap<String, String>, event: &BatchEvent) -> bool {
    event
        .etag
        .as_ref()
        .is_some_and(|etag| echoes.get(&event.provider_id) == Some(etag))
}

/// Publish how `row` ended, for the schedule module.
pub(super) async fn announce(
    state: &AppState,
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    row: &Recorded,
    (outcome, reason, written): (CalendarOutcome, Option<CalendarReason>, Option<&Written>),
) -> Result<(), JobError> {
    state
        .events
        .publish(
            tx,
            &CalendarChangeCompleted {
                workspace: workspace.to_string(),
                request: row.request_id,
                calendar: row.calendar_id,
                event: row.event_id,
                outcome,
                provider_event_id: written.map(|w| w.provider_event_id.clone()),
                etag: written.and_then(|w| w.etag.clone()),
                reason,
            },
        )
        .await?;
    state
        .tracker
        .track(
            tx,
            &tracking::CalendarChangeCompleted {
                workspace,
                request_id: row.request_id,
                calendar_id: row.calendar_id,
                event_id: row.event_id,
                stream_id: Some(*row.stream_id.as_uuid()),
                change: tracked::calendar_change(&row.change),
                outcome: tracked::calendar_outcome(outcome),
                reason: reason.map(tracked::calendar_reason),
            },
        )
        .await?;
    Ok(())
}

/// Record a batch of `calendar`, hold its blob and announce it.
async fn store_batch(
    tx: &mut Transaction<'static, Postgres>,
    state: &AppState,
    (workspace, stream, connection): (WorkspaceId, StreamId, ConnectionId),
    calendar: Uuid,
    batch: &BlobRef,
    changes: usize,
    now: DateTime<Utc>,
) -> Result<(), JobError> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO connectors.batches
             (id, workspace_id, stream_id, batch_hash, batch_size, cursor, changes, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(id)
    .bind(workspace)
    .bind(stream)
    .bind(batch.hash.to_string())
    .bind(i64::try_from(batch.size).unwrap_or(i64::MAX))
    .bind(format!("calendar:{calendar}"))
    .bind(i32::try_from(changes).unwrap_or(i32::MAX))
    .bind(now)
    .execute(&mut **tx)
    .await?;
    let key = workspace.to_string();
    state
        .blobs
        .attach(tx, &key, batch, &BlobOwner::new(BATCH_OWNER, id))
        .await?;
    state
        .events
        .publish(
            tx,
            &CalendarChanged {
                workspace: key,
                connection,
                batch: batch.clone(),
            },
        )
        .await?;
    Ok(())
}
