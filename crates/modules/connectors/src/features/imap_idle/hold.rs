//! One connection held in IDLE, from its claim to its end: sign in, open
//! the Inbox read-only, wait in IDLE, ask for runs, renew IDLE; reconnect
//! after a failure with a backoff, through the guard and with the password
//! opened again each time.
//!
//! Nothing the server says is logged: ids, stable codes and counts only.

use std::{
    collections::hash_map::RandomState,
    hash::{BuildHasher as _, Hasher as _},
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use campbooks_kernel::{db::BeginFor as _, tenancy::UserId};
use secrecy::SecretString;
use tokio::{sync::watch, time::Instant};

use super::{
    Shared,
    lease::{self, Row},
};
use crate::{
    domain::{
        credentials::{self, TokenError},
        health, imap_servers, schedule,
    },
    imap::{
        Refusal, Server,
        session::{Heard, IdleWait, Limits, Session, is_change},
    },
};

/// The largest response read in IDLE.
pub(crate) const MAX_RESPONSE: usize = 64 * 1024;

/// The most responses one IDLE period may bring.
pub(crate) const MAX_RESPONSES: usize = 1_000;

/// The most bytes one IDLE period may bring.
pub(crate) const MAX_BYTES: usize = 1024 * 1024;

/// How a connection's session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ended {
    /// The pool asked it to stop (lease lost, shutdown).
    Stopped,
    /// The connection may not IDLE (any more): its row is withdrawn.
    Withdrawn(&'static str),
    /// Its password was refused once too often: it needs the member.
    NeedsReauth,
    /// It failed with `code`; `idled` when an IDLE period completed first.
    Failed { code: &'static str, idled: bool },
}

/// What a session needs of its connection, read at each (re)connect.
struct Target {
    username: String,
    server: Server,
    password: SecretString,
    granted_by: UserId,
}

/// Hold `row` in IDLE until `stop` is set, the row may no longer IDLE, or
/// it failed too often in a row. Its connection.
pub(super) async fn hold(
    shared: Arc<Shared>,
    row: Row,
    mut stop: watch::Receiver<bool>,
) -> crate::domain::model::ConnectionId {
    let timing = shared.timing;
    let mut debounce = Debounce::new(timing.debounce);
    let mut failures = 0_u32;
    loop {
        if *stop.borrow() {
            break;
        }
        let ended = Box::pin(session(&shared, &row, &mut stop, &mut debounce)).await;
        match ended {
            Ended::Stopped | Ended::NeedsReauth => break,
            Ended::Withdrawn(code) => {
                tracing::info!(
                    workspace = %row.workspace, connection = %row.connection, code,
                    "IMAP IDLE withdrawn; the mailbox is polled"
                );
                withdraw(&shared, &row).await;
                break;
            }
            Ended::Failed { code, idled } => {
                if idled {
                    failures = 0;
                }
                failures = failures.saturating_add(1);
                tracing::info!(
                    workspace = %row.workspace, connection = %row.connection, code, failures,
                    "IMAP IDLE session ended"
                );
                if failures >= timing.give_up_after {
                    give_up(&shared, &row).await;
                    break;
                }
                let wait = backoff(failures, timing.backoff, timing.backoff_max, random());
                tokio::select! {
                    () = tokio::time::sleep(wait) => {}
                    () = stopped(&mut stop) => break,
                }
            }
        }
    }
    row.connection
}

/// Resolves once the pool asks the session to stop (or is gone).
async fn stopped(stop: &mut watch::Receiver<bool>) {
    let _ = stop.wait_for(|stopped| *stopped).await;
}

/// One session: load, sign in, then [`idle`] until it ends.
async fn session(
    shared: &Shared,
    row: &Row,
    stop: &mut watch::Receiver<bool>,
    debounce: &mut Debounce,
) -> Ended {
    let target = match load(shared, row).await {
        Ok(Some(target)) => target,
        Ok(None) => return Ended::Withdrawn("imap_idle_not_eligible"),
        Err(err) => {
            tracing::warn!(
                connection = %row.connection,
                error = %campbooks_kernel::redact::error(&err),
                "could not load an IMAP IDLE connection"
            );
            return Ended::Failed {
                code: "database",
                idled: false,
            };
        }
    };
    let imap = &shared.deps.imap;
    let Target {
        username,
        server,
        password,
        granted_by,
    } = target;
    // Through the guard, the mail ports and TLS, as a sync run signs in.
    let opened = tokio::time::timeout(
        imap.timeout(),
        Session::open(imap, &server, &username, &password),
    )
    .await
    .unwrap_or(Err(Refusal::ImapUnreachable));
    // The password is never kept past the sign-in.
    drop(password);
    let mut session = match opened {
        Ok(session) => session,
        Err(Refusal::ImapLoginRefused) => return refused(shared, row, granted_by).await,
        Err(refusal) => {
            return Ended::Failed {
                code: refusal.code(),
                idled: false,
            };
        }
    };
    let (ended, polite) = idle(shared, row, &mut session, stop, debounce).await;
    if polite {
        session.logout().await;
    }
    ended
}

/// The bounds of a short command.
fn small(shared: &Shared) -> Limits {
    Limits {
        response: MAX_RESPONSE,
        responses: MAX_RESPONSES,
        total: MAX_BYTES,
        time: shared.deps.imap.timeout(),
    }
}

/// Open the Inbox and wait in IDLE, renewing it, until the session ends.
/// How it ended, and whether it may still log out (not mid-answer).
#[allow(
    clippy::too_many_lines,
    reason = "one IDLE conversation read top to bottom: open, wait, renew"
)]
async fn idle(
    shared: &Shared,
    row: &Row,
    session: &mut Session,
    stop: &mut watch::Receiver<bool>,
    debounce: &mut Debounce,
) -> (Ended, bool) {
    let limits = small(shared);
    let failed = |code: &'static str, idled: bool| Ended::Failed { code, idled };
    let capabilities = match session.capabilities(limits).await {
        Ok(capabilities) => capabilities,
        Err(err) => return (failed(err.code(), false), false),
    };
    if !capabilities.iter().any(|c| c == "IDLE") {
        return (Ended::Withdrawn("imap_idle_unsupported"), true);
    }
    match session.command("EXAMINE INBOX", limits, |_| {}).await {
        Ok(done) if done.is_ok() => {}
        Ok(done) if done.is_busy() => return (failed("imap_busy", false), true),
        Ok(_) => return (failed("imap_refused", false), true),
        Err(err) => return (failed(err.code(), false), false),
    }
    let mut idled = false;
    loop {
        let tag = match session.idle(limits).await {
            Ok(Some(tag)) => tag,
            Ok(None) => return (Ended::Withdrawn("imap_idle_refused"), true),
            Err(err) => return (failed(err.code(), idled), false),
        };
        let refresh_at = Instant::now() + shared.timing.refresh;
        let mut responses = 0_usize;
        let mut bytes = 0_usize;
        let next = loop {
            let wake = debounce.due().map_or(refresh_at, |due| due.min(refresh_at));
            match session.idle_wait(wake, stopped(stop)).await {
                Err(err) => return (failed(err.code(), idled), false),
                Ok(IdleWait::Stopped) => break Next::Stop,
                Ok(IdleWait::TimedOut) => {
                    if debounce.take_due(Instant::now()) {
                        match ask(shared, row).await {
                            Standing::Eligible => {}
                            standing => break Next::End(standing),
                        }
                    }
                    if Instant::now() >= refresh_at {
                        break Next::Refresh;
                    }
                }
                Ok(IdleWait::Ready) => match session.idle_read(MAX_RESPONSE).await {
                    Err(err) => return (failed(err.code(), idled), false),
                    Ok((heard, size)) => {
                        responses += 1;
                        bytes = bytes.saturating_add(size);
                        if responses > MAX_RESPONSES || bytes > MAX_BYTES {
                            tracing::warn!(
                                workspace = %row.workspace, connection = %row.connection,
                                responses, bytes, code = "imap_idle_flood",
                                "an IMAP server said too much in IDLE; the session is dropped"
                            );
                            return (failed("imap_idle_flood", false), false);
                        }
                        if heard == Heard::Changed && debounce.heard(Instant::now()) {
                            match ask(shared, row).await {
                                Standing::Eligible => {}
                                standing => break Next::End(standing),
                            }
                        }
                    }
                },
            }
        };
        // End this IDLE; what the server still had to say counts too.
        let mut changed = false;
        match session
            .idle_done(&tag, limits, |untagged| changed |= is_change(&untagged))
            .await
        {
            Ok(done) if done.is_ok() => idled = true,
            Ok(_) => return (failed("imap_refused", idled), true),
            Err(err) => return (failed(err.code(), idled), false),
        }
        match next {
            Next::Stop => return (Ended::Stopped, true),
            Next::End(standing) => return (standing.ended(), true),
            // Access may have been cut since (paused, needs the member, a new
            // password, the lease lost): never IDLE past a renewal for it.
            Next::Refresh => match standing(shared, row).await {
                Ok(Standing::Eligible) | Err(_) => {}
                Ok(standing) => return (standing.ended(), true),
            },
        }
        // The keepalive: traffic both ways, and a tagged answer that proves
        // the server is still there.
        match session
            .command("NOOP", limits, |untagged| changed |= is_change(&untagged))
            .await
        {
            Ok(done) if done.is_ok() => {}
            Ok(_) => return (failed("imap_refused", idled), true),
            Err(err) => return (failed(err.code(), idled), false),
        }
        if changed && debounce.heard(Instant::now()) {
            match ask(shared, row).await {
                Standing::Eligible => {}
                standing => return (standing.ended(), true),
            }
        }
    }
}

/// What an IDLE period ended with.
enum Next {
    /// The pool asked the session to stop.
    Stop,
    /// Time to renew IDLE.
    Refresh,
    /// The connection may no longer be held (found when asking for a run).
    End(Standing),
}

/// Whether a held connection may still be held in IDLE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
    /// Yes.
    Eligible,
    /// No: paused, waiting for the member, its server without IDLE, or its
    /// row withdrawn (a new password, a quiet mailbox).
    Ineligible,
    /// This process no longer holds its lease (another may).
    Lost,
}

impl Standing {
    /// How a session that finds it so ends.
    const fn ended(self) -> Ended {
        match self {
            Self::Eligible | Self::Lost => Ended::Stopped,
            Self::Ineligible => Ended::Withdrawn("imap_idle_not_eligible"),
        }
    }
}

/// In the connection's workspace: whether `row` may still be held in IDLE
/// by this process.
async fn standing(shared: &Shared, row: &Row) -> campbooks_kernel::Result<Standing> {
    let mut tx = shared.state.db.begin_for(row.workspace).await?;
    let standing = standing_in(&mut tx, shared, row).await?;
    tx.commit().await?;
    Ok(standing)
}

/// [`standing`], in `conn` (the connection's workspace).
async fn standing_in(
    conn: &mut sqlx::PgConnection,
    shared: &Shared,
    row: &Row,
) -> campbooks_kernel::Result<Standing> {
    let found: Option<(bool, bool)> = sqlx::query_as(
        "SELECT NOT s.paused AND c.health <> 'needs_reauth'
                    AND coalesce((s.cursor->>'idle')::boolean, true) AND l.offered,
                coalesce(l.holder = $3, false)
           FROM connectors.streams s
           JOIN connectors.connections c ON c.id = s.connection_id
           JOIN connectors.imap_idle_leases l ON l.connection_id = c.id
          WHERE s.id = $1 AND c.id = $2",
    )
    .bind(row.stream)
    .bind(row.connection)
    .bind(shared.holder)
    .fetch_optional(conn)
    .await?;
    Ok(match found {
        None | Some((false, _)) => Standing::Ineligible,
        Some((true, false)) => Standing::Lost,
        Some((true, true)) => Standing::Eligible,
    })
}

/// In the connection's workspace: its stream (not paused), its connection
/// (IMAP, not waiting for the member), its server, username and password,
/// opened. `None` when it may not IDLE.
async fn load(shared: &Shared, row: &Row) -> campbooks_kernel::Result<Option<Target>> {
    let Some(cipher) = shared.deps.cipher.as_deref() else {
        return Ok(None);
    };
    let mut tx = shared.state.db.begin_for(row.workspace).await?;
    let state: Option<(String, String, UserId, bool)> = sqlx::query_as(
        "SELECT c.provider, c.health, c.granted_by, s.paused
           FROM connectors.streams s JOIN connectors.connections c ON c.id = s.connection_id
          WHERE s.id = $1 AND c.id = $2",
    )
    .bind(row.stream)
    .bind(row.connection)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((provider, health, granted_by, paused)) = state else {
        tx.commit().await?;
        return Ok(None);
    };
    if provider != "imap" || health == "needs_reauth" || paused {
        tx.commit().await?;
        return Ok(None);
    }
    let account = imap_servers::imap_server(&mut tx, row.connection).await?;
    let password = credentials::password(&mut tx, cipher, row.connection).await;
    tx.commit().await?;
    let password = match password {
        Ok(password) => password,
        Err(TokenError::Database(err)) => return Err(err),
        Err(TokenError::NeedsReauth(_) | TokenError::Provider(_)) => return Ok(None),
    };
    Ok(account.map(|(username, server)| Target {
        username,
        server,
        password,
        granted_by,
    }))
}

/// Ask for a run of `row`'s stream now, in its workspace, while it may
/// still be held in IDLE: a run already pending absorbs it. Whether it may
/// (on a database error, as if it may: the renewal settles it).
async fn ask(shared: &Shared, row: &Row) -> Standing {
    shared.asked.fetch_add(1, Ordering::Relaxed);
    let now = shared.deps.clock.now();
    let asked = async {
        let mut tx = shared.state.db.begin_for(row.workspace).await?;
        let standing = standing_in(&mut tx, shared, row).await?;
        if standing == Standing::Eligible {
            schedule::run_now(&mut tx, row.workspace, row.stream, now).await?;
        }
        tx.commit().await?;
        Ok::<Standing, campbooks_kernel::Error>(standing)
    };
    match asked.await {
        Ok(standing) => standing,
        Err(err) => {
            tracing::warn!(
                stream = %row.stream,
                error = %campbooks_kernel::redact::error(&err),
                "could not ask for a run after IMAP IDLE"
            );
            Standing::Eligible
        }
    }
}

/// The server refused the password: count it with the sync runs' refusals;
/// the last one allowed sets `needs_reauth` (which withdraws the row).
async fn refused(shared: &Shared, row: &Row, granted_by: UserId) -> Ended {
    let now = shared.deps.clock.now();
    let counted = async {
        let mut tx = shared.state.db.begin_for(row.workspace).await?;
        let refusals = health::count_login_refusal(&mut tx, row.connection, row.stream).await?;
        let reauth = refusals >= health::LOGIN_REFUSALS;
        if reauth {
            health::needs_reauth(
                &mut tx,
                &shared.state.events,
                &row.workspace.to_string(),
                row.connection,
                granted_by,
                "imap_login_refused",
                now,
            )
            .await?;
        }
        tx.commit().await?;
        Ok::<bool, campbooks_kernel::Error>(reauth)
    };
    match counted.await {
        Ok(true) => Ended::NeedsReauth,
        Ok(false) => Ended::Failed {
            code: Refusal::ImapLoginRefused.code(),
            idled: false,
        },
        Err(err) => {
            tracing::warn!(
                connection = %row.connection,
                error = %campbooks_kernel::redact::error(&err),
                "could not count a refused IMAP sign-in"
            );
            Ended::Failed {
                code: Refusal::ImapLoginRefused.code(),
                idled: false,
            }
        }
    }
}

/// Withdraw `row` (it may not IDLE): in its workspace.
async fn withdraw(shared: &Shared, row: &Row) {
    let withdrawn = async {
        let mut tx = shared.state.db.begin_for(row.workspace).await?;
        lease::withdraw(&mut tx, row.connection).await?;
        tx.commit().await?;
        Ok::<(), campbooks_kernel::Error>(())
    };
    if let Err(err) = withdrawn.await {
        tracing::warn!(
            connection = %row.connection,
            error = %campbooks_kernel::redact::error(&err),
            "could not withdraw an IMAP IDLE lease"
        );
    }
}

/// Give `row`'s lease up for the cooldown: polled meanwhile.
async fn give_up(shared: &Shared, row: &Row) {
    let cooldown = i32::try_from(shared.timing.cooldown.as_secs()).unwrap_or(i32::MAX);
    tracing::info!(
        workspace = %row.workspace, connection = %row.connection,
        "IMAP IDLE given up for a while; the mailbox is polled"
    );
    if let Err(err) =
        lease::give_up(&shared.state.db, shared.holder, row.connection, cooldown).await
    {
        tracing::warn!(
            connection = %row.connection,
            error = %campbooks_kernel::redact::error(&err),
            "could not give up an IMAP IDLE lease; it expires on its own"
        );
    }
}

/// The wait after `failures` failures in a row: `base` doubled per failure
/// after the first, at most `max`, drawn from the upper half by `random`.
pub(crate) fn backoff(failures: u32, base: Duration, max: Duration, random: u64) -> Duration {
    let doublings = failures.saturating_sub(1).min(16);
    let cap = base.saturating_mul(1_u32 << doublings).min(max);
    let half = cap / 2;
    let spread = u64::try_from(half.as_millis()).unwrap_or(u64::MAX);
    half + Duration::from_millis(random % spread.saturating_add(1))
}

/// A random number from the standard library's per-instance hash keys.
fn random() -> u64 {
    RandomState::new().build_hasher().finish()
}

/// At most one run asked for per interval: the first change asks at once;
/// changes within the interval after it fold into one more ask at its end.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Debounce {
    every: Duration,
    last: Option<Instant>,
    pending: bool,
}

impl Debounce {
    pub(crate) const fn new(every: Duration) -> Self {
        Self {
            every,
            last: None,
            pending: false,
        }
    }

    /// A change was heard at `now`: whether to ask now.
    pub(crate) fn heard(&mut self, now: Instant) -> bool {
        if self.last.is_none_or(|last| now >= last + self.every) {
            self.last = Some(now);
            self.pending = false;
            true
        } else {
            self.pending = true;
            false
        }
    }

    /// When the folded ask is due, if there is one.
    pub(crate) fn due(&self) -> Option<Instant> {
        self.last
            .filter(|_| self.pending)
            .map(|last| last + self.every)
    }

    /// Whether the folded ask is due at `now` (it is then taken).
    pub(crate) fn take_due(&mut self, now: Instant) -> bool {
        if self.due().is_some_and(|due| now >= due) {
            self.last = Some(now);
            self.pending = false;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_ask_at_once_then_once_more_per_interval() {
        let start = Instant::now();
        let at = |seconds: u64| start + Duration::from_secs(seconds);
        let mut debounce = Debounce::new(Duration::from_secs(30));
        assert_eq!(debounce.due(), None);
        // The first change asks at once.
        assert!(debounce.heard(at(0)));
        assert_eq!(debounce.due(), None);
        // A burst within the interval: nothing more now, one ask due at its end.
        for second in 1..20 {
            assert!(!debounce.heard(at(second)));
        }
        assert_eq!(debounce.due(), Some(at(30)));
        assert!(!debounce.take_due(at(29)));
        assert!(debounce.take_due(at(30)));
        assert!(!debounce.take_due(at(31)));
        assert_eq!(debounce.due(), None);
        // A change after a quiet interval asks at once again.
        assert!(debounce.heard(at(61)));
    }

    #[test]
    fn backoff_doubles_with_jitter_up_to_its_ceiling() {
        let base = Duration::from_secs(5);
        let max = Duration::from_mins(15);
        for random in [0, 1, 7_919, u64::MAX] {
            let first = backoff(1, base, max, random);
            assert!(
                first >= Duration::from_millis(2_500) && first <= base,
                "{first:?}"
            );
            let third = backoff(3, base, max, random);
            assert!(
                third >= Duration::from_secs(10) && third <= Duration::from_secs(20),
                "{third:?}"
            );
            let late = backoff(40, base, max, random);
            assert!(late >= max / 2 && late <= max, "{late:?}");
        }
        assert_eq!(backoff(1, Duration::ZERO, max, 9), Duration::ZERO);
    }
}
