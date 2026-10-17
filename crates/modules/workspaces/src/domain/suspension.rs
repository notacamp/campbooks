//! A user's suspension, as this module last announced it in their
//! workspaces (campbooks-cloud#147, #197).
//!
//! The newest change is kept on the user's row (`workspaces.users.suspended`,
//! `suspension_seq`, `suspension_at`) and read and written only under that
//! row's lock ([`lock`]), which every path that announces a change or changes
//! the user's memberships takes first:
//!
//! - announcing a change (`workspaces.announce_member_suspended` /
//!   `…_unsuspended`) records it, then lists the memberships and announces
//!   it in each;
//! - joining a workspace (accepting an invitation) announces the recorded
//!   change in the workspace joined;
//! - leaving one (or being removed) takes the lock too, so the memberships
//!   the announcement lists are the ones that hold when it commits.
//!
//! So whichever commits first, every workspace the user belongs to hears the
//! newest change: a member who re-joins while being suspended is paused
//! there too, and one who comes back after a lift that no longer reached
//! the workspace is told of the lift on joining.
//!
//! The lock is `FOR NO KEY UPDATE`, taken before any of the workspace's
//! locks: it waits only for another of these, never for the key-share locks
//! that a foreign key to the user takes (a security event naming them, a
//! membership), so a transaction holding a workspace's lock and recording
//! such a row never waits on it, and no two of these paths wait on each
//! other the other way round.

use campbooks_kernel::{EventBus, tenancy::UserId};
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, Postgres, Transaction};

use crate::events::{MemberSuspended, MemberUnsuspended};

/// A change to an account's suspension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Change {
    /// Suspended (`true`) or lifted.
    pub suspended: bool,
    /// The account's count of suspensions and lifts (identity's `seq`);
    /// `None` for an event published before it existed.
    pub seq: Option<i64>,
    /// When it took effect.
    pub at: DateTime<Utc>,
}

impl Change {
    /// Whether `self` is older than `known`, the change recorded: a lower
    /// `seq`, or no `seq` against one that has it (published before the
    /// counter existed, so before it), or, neither having one, an earlier
    /// `at`. The same change again is not older: announcing it again is
    /// harmless, and it repairs a delivery that was lost downstream.
    pub fn is_older_than(&self, known: &Self) -> bool {
        match (self.seq, known.seq) {
            (Some(seq), Some(known_seq)) => seq < known_seq,
            (Some(_), None) => false,
            (None, Some(_)) => true,
            (None, None) => self.at < known.at,
        }
    }
}

/// The row [`lock`] reads.
type Row = (UserId, bool, Option<i64>, Option<DateTime<Utc>>);

fn change_of(row: &Row) -> Option<Change> {
    let (_, suspended, seq, at) = *row;
    at.map(|at| Change { suspended, seq, at })
}

/// Lock `user`'s row until the transaction ends and read the change last
/// announced for them; `None` when none was (or there is no such user).
pub async fn lock(conn: &mut PgConnection, user: UserId) -> Result<Option<Change>, sqlx::Error> {
    let row: Option<Row> = sqlx::query_as(
        "SELECT id, suspended, suspension_seq, suspension_at
           FROM workspaces.users WHERE id = $1 FOR NO KEY UPDATE",
    )
    .bind(user)
    .fetch_optional(conn)
    .await?;
    Ok(row.as_ref().and_then(change_of))
}

/// Like [`lock`], for the user whose identity subject is `subject`: `None`
/// when no such user signed in; else the user and their change, if any.
pub async fn lock_subject(
    conn: &mut PgConnection,
    subject: &str,
) -> Result<Option<(UserId, Option<Change>)>, sqlx::Error> {
    let row: Option<Row> = sqlx::query_as(
        "SELECT id, suspended, suspension_seq, suspension_at
           FROM workspaces.users WHERE idp_subject = $1 FOR NO KEY UPDATE",
    )
    .bind(subject)
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|row| (row.0, change_of(&row))))
}

/// Record `change` as the newest for `user` (whose row the transaction
/// locked).
pub async fn record(
    conn: &mut PgConnection,
    user: UserId,
    change: Change,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE workspaces.users
            SET suspended = $2, suspension_seq = $3, suspension_at = $4
          WHERE id = $1",
    )
    .bind(user)
    .bind(change.suspended)
    .bind(change.seq)
    .bind(change.at)
    .execute(conn)
    .await?;
    Ok(())
}

/// Publish `change` for `user` in `workspace`, from `tx`, a transaction
/// working in that workspace.
pub async fn announce(
    events: &EventBus,
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    user: UserId,
    change: Change,
) -> Result<(), sqlx::Error> {
    let workspace = workspace.to_owned();
    let Change { suspended, seq, at } = change;
    if suspended {
        events
            .publish(
                tx,
                &MemberSuspended {
                    workspace,
                    user,
                    at,
                    seq,
                },
            )
            .await?;
    } else {
        events
            .publish(
                tx,
                &MemberUnsuspended {
                    workspace,
                    user,
                    at,
                    seq,
                },
            )
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use super::Change;

    #[test]
    fn the_counter_orders_changes_and_the_clock_only_those_without_it() {
        let now = Utc::now();
        let change = |suspended, seq, at| Change { suspended, seq, at };
        // A lift counted later wins even with an earlier clock.
        let suspended = change(true, Some(3), now);
        let lifted = change(false, Some(4), now - Duration::seconds(30));
        assert!(suspended.is_older_than(&lifted));
        assert!(!lifted.is_older_than(&suspended));
        // The same change again is not older (a replay).
        assert!(!lifted.is_older_than(&lifted));
        // Without a count: published before it existed, so older.
        let old = change(true, None, now + Duration::hours(1));
        assert!(old.is_older_than(&lifted));
        assert!(!lifted.is_older_than(&old));
        // Neither has one: the clock decides.
        let older = change(false, None, now);
        assert!(older.is_older_than(&old));
        assert!(!old.is_older_than(&older));
    }
}
