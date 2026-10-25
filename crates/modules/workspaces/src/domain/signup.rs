//! The instance's signup policy: who may create their first workspace.
//!
//! - `open`, `invite_only`: as configured.
//! - `approval`: a refused attempt files one signup request per user
//!   (idempotent); an administrator approves or rejects it.
//! - `code`: a code from `CAMPBOOKS_SIGNUP__CODES` (reusable, the legacy
//!   path) or one an administrator minted (stored as a SHA-256 digest, spent
//!   by one atomic update in the transaction that creates the workspace).

use aws_lc_rs::{constant_time, digest, rand};
use campbooks_kernel::{
    AppState, Error, Result, admin::AdminGrant, config::SignupPolicy, db::begin_admin,
    tenancy::UserId,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;

use super::{
    db::{begin_as_user, set_user},
    model::SignupCodeId,
    security::{self, SecurityKind},
    tracked,
};
use crate::{MESSAGES, tracking};

/// Where a user's signup request stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RequestStatus {
    /// Waiting for an administrator.
    Pending,
    /// Approved: the user may create a workspace.
    Approved,
    /// Turned down.
    Rejected,
}

impl RequestStatus {
    /// The stored name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }

    /// The status stored as `name`.
    pub fn parse(name: &str) -> Result<Self> {
        match name {
            "pending" => Ok(Self::Pending),
            "approved" => Ok(Self::Approved),
            "rejected" => Ok(Self::Rejected),
            other => Err(Error::internal(format!(
                "unknown signup request status `{other}`"
            ))),
        }
    }
}

/// A signup code that must be spent in the transaction creating the
/// workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeDigest(Vec<u8>);

/// What the signup policy decided for one attempt to create a workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    /// Go ahead.
    Admitted,
    /// Go ahead if this minted code can still be spent, atomically with the
    /// creation (see [`redeem`]).
    Redeem(CodeDigest),
}

/// Whether the policy applies to `user` at all: members and the instance's
/// first user are never held to it.
pub async fn is_exempt(conn: &mut PgConnection, user: UserId) -> Result<bool> {
    let row = sqlx::query!(
        r#"
        SELECT EXISTS (
                   SELECT 1 FROM workspaces.members m
                     JOIN workspaces.workspaces w ON w.id = m.workspace_id
                    WHERE m.user_id = $1 AND w.deleted_at IS NULL
               ) AS "is_member!",
               coalesce((SELECT u.id = $1 FROM workspaces.users u
                          ORDER BY u.created_at, u.id LIMIT 1), false) AS "is_first_user!"
        "#,
        user as UserId,
    )
    .fetch_one(conn)
    .await?;
    Ok(row.is_member || row.is_first_user)
}

/// The status of `user`'s signup request, if they filed one.
pub async fn request_status(
    conn: &mut PgConnection,
    user: UserId,
) -> Result<Option<RequestStatus>> {
    sqlx::query_scalar!(
        "SELECT status FROM workspaces.signup_requests WHERE user_id = $1",
        user as UserId
    )
    .fetch_optional(conn)
    .await?
    .map(|status| RequestStatus::parse(&status))
    .transpose()
}

/// Apply the signup policy to `user` creating a workspace with `code`.
pub async fn admit(state: &AppState, user: UserId, code: Option<&str>) -> Result<Admission> {
    let signup = &state.config.signup;
    if signup.policy == SignupPolicy::Open {
        return Ok(Admission::Admitted);
    }
    let mut tx = begin_as_user(&state.db, user).await?;
    // Members may create more workspaces; the instance's first user
    // bootstraps it under any policy.
    if is_exempt(&mut tx, user).await? {
        return Ok(Admission::Admitted);
    }
    match signup.policy {
        SignupPolicy::Open => Ok(Admission::Admitted),
        SignupPolicy::InviteOnly => Err(Error::Denied(MESSAGES.reason("signup_invite_only"))),
        SignupPolicy::Approval => match request_status(&mut tx, user).await? {
            Some(RequestStatus::Approved) => Ok(Admission::Admitted),
            Some(RequestStatus::Rejected) => Err(Error::Denied(MESSAGES.reason("signup_rejected"))),
            Some(RequestStatus::Pending) => {
                Err(Error::Denied(MESSAGES.reason("signup_requires_approval")))
            }
            None => {
                let filed = sqlx::query!(
                    "INSERT INTO workspaces.signup_requests (user_id) VALUES ($1)
                     ON CONFLICT (user_id) DO NOTHING",
                    user as UserId
                )
                .execute(&mut *tx)
                .await?
                .rows_affected();
                if filed > 0 {
                    security::record(
                        &mut tx,
                        None,
                        Some(user),
                        SecurityKind::SignupRequested,
                        serde_json::json!({}),
                    )
                    .await?;
                    let signup_request = tracked::signup_request(&mut tx, user).await?;
                    state
                        .tracker
                        .track(&mut tx, &tracking::SignupRequestFiled { signup_request })
                        .await?;
                }
                tx.commit().await?;
                Err(Error::Denied(MESSAGES.reason("signup_requires_approval")))
            }
        },
        SignupPolicy::Code => {
            let Some(code) = code.map(str::trim).filter(|code| !code.is_empty()) else {
                return Err(Error::Denied(MESSAGES.reason("signup_code_invalid")));
            };
            if matches_any(code, &signup.codes()) {
                Ok(Admission::Admitted)
            } else {
                Ok(Admission::Redeem(digest_of(code)))
            }
        }
    }
}

/// Spend one use of the minted code with `digest`, in the transaction that
/// creates the workspace: one atomic update, so two attempts at the last use
/// cannot both win. Returns the code's id; a code that is unknown, revoked,
/// expired or used up is the `signup_code_invalid` refusal.
pub async fn redeem(conn: &mut PgConnection, digest: &CodeDigest) -> Result<SignupCodeId> {
    sqlx::query_scalar!(
        r#"UPDATE workspaces.signup_codes
              SET uses = uses + 1
            WHERE digest = $1
              AND revoked_at IS NULL
              AND uses < max_uses
              AND (expires_at IS NULL OR expires_at > now())
        RETURNING id AS "id: SignupCodeId""#,
        digest.0,
    )
    .fetch_optional(conn)
    .await?
    .ok_or_else(|| Error::Denied(MESSAGES.reason("signup_code_invalid")))
}

/// Whether `code` equals one of `codes`, comparing digests in constant time
/// and checking every code.
pub fn matches_any(code: &str, codes: &[&str]) -> bool {
    let given = digest::digest(&digest::SHA256, code.trim().as_bytes());
    codes.iter().fold(false, |found, candidate| {
        let expected = digest::digest(&digest::SHA256, candidate.as_bytes());
        constant_time::verify_slices_are_equal(given.as_ref(), expected.as_ref()).is_ok() | found
    })
}

/// Digits and letters a minted code is made of: no `0`, `1`, `I` or `O`.
const ALPHABET: &[u8; 32] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
/// Groups of four characters in a minted code: 80 bits of randomness.
const GROUPS: usize = 4;

/// A new code, like `K7QM-2XWP-9DHE-RT4A`, and its digest.
pub fn new_code() -> Result<(String, Vec<u8>)> {
    let mut bytes = [0_u8; GROUPS * 4];
    rand::fill(&mut bytes).map_err(|_| Error::internal("no randomness for a signup code"))?;
    let mut code = String::with_capacity(GROUPS * 5);
    for (index, byte) in bytes.iter().enumerate() {
        if index > 0 && index % 4 == 0 {
            code.push('-');
        }
        code.push(char::from(ALPHABET[usize::from(byte & 31)]));
    }
    let digest = digest_of(&code).0;
    Ok((code, digest))
}

/// The digest of a code as typed: whitespace and dashes are ignored and
/// case does not matter, so a code survives being copied around.
pub fn digest_of(code: &str) -> CodeDigest {
    let normalized: String = code
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .flat_map(char::to_uppercase)
        .collect();
    CodeDigest(
        digest::digest(&digest::SHA256, normalized.as_bytes())
            .as_ref()
            .to_vec(),
    )
}

/// One signup request, as an administrator sees it: who asked and where it
/// stands. No content, only the account's own name and address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SignupRequestView {
    /// The requesting user (also the request's id).
    pub user_id: UserId,
    /// Their display name, when their sign-in carried one.
    pub display_name: Option<String>,
    /// Their email address, when their sign-in carried one.
    pub email: Option<String>,
    /// Where the request stands.
    pub status: RequestStatus,
    /// When they first asked.
    pub requested_at: DateTime<Utc>,
    /// When an administrator decided.
    pub decided_at: Option<DateTime<Utc>>,
    /// The deciding administrator's account id.
    pub decided_by: Option<String>,
    /// Why it was turned down, when the administrator said.
    pub reason: Option<String>,
}

/// The path of one request: `/admin/v1/workspaces/signup-requests/{user_id}/…`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct SignupRequestPath {
    /// The requesting user.
    pub user_id: UserId,
}

/// What a signup code can still do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CodeState {
    /// It can be redeemed.
    Active,
    /// Every use is spent.
    UsedUp,
    /// Its expiry has passed.
    Expired,
    /// An administrator revoked it.
    Revoked,
}

impl CodeState {
    /// The state of a code (`revoked`, its uses and expiry) at `now`.
    pub fn of(
        revoked: bool,
        uses: i32,
        max_uses: i32,
        expires_at: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
    ) -> Self {
        if revoked {
            Self::Revoked
        } else if uses >= max_uses {
            Self::UsedUp
        } else if expires_at.is_some_and(|at| at <= now) {
            Self::Expired
        } else {
            Self::Active
        }
    }
}

/// One minted signup code. The code itself is never here: only its
/// administrators saw it, once, when it was minted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SignupCodeView {
    /// Its id.
    pub id: SignupCodeId,
    /// What the administrator called it.
    pub label: String,
    /// What it can still do.
    pub state: CodeState,
    /// How many times it can be redeemed.
    pub max_uses: i32,
    /// How many times it was.
    pub uses: i32,
    /// After this it cannot be redeemed.
    pub expires_at: Option<DateTime<Utc>>,
    /// The administrator who minted it (their account id).
    pub created_by: String,
    /// When.
    pub created_at: DateTime<Utc>,
    /// When it was revoked.
    pub revoked_at: Option<DateTime<Utc>>,
}

impl SignupCodeView {
    /// The view of a stored code, judged at `now`.
    pub fn at(row: &CodeRow, now: DateTime<Utc>) -> Self {
        let state = CodeState::of(
            row.revoked_at.is_some(),
            row.uses,
            row.max_uses,
            row.expires_at,
            now,
        );
        Self {
            id: row.id,
            label: row.label.clone(),
            state,
            max_uses: row.max_uses,
            uses: row.uses,
            expires_at: row.expires_at,
            created_by: row.created_by.clone(),
            created_at: row.created_at,
            revoked_at: row.revoked_at,
        }
    }
}

/// A `workspaces.signup_codes` row without its digest.
#[derive(Debug, Clone)]
pub struct CodeRow {
    /// Its id.
    pub id: SignupCodeId,
    /// Its label.
    pub label: String,
    /// Its use limit.
    pub max_uses: i32,
    /// Its uses so far.
    pub uses: i32,
    /// Its expiry.
    pub expires_at: Option<DateTime<Utc>>,
    /// Who minted it.
    pub created_by: String,
    /// When.
    pub created_at: DateTime<Utc>,
    /// When it was revoked.
    pub revoked_at: Option<DateTime<Utc>>,
}

/// The signup request of `user` as an administrator sees it.
pub async fn request_view(
    conn: &mut PgConnection,
    user: UserId,
) -> Result<Option<SignupRequestView>> {
    let Some(row) = sqlx::query!(
        r#"SELECT r.user_id AS "user_id: UserId", u.display_name, u.email, r.status,
                  r.requested_at, r.decided_at, r.decided_by, r.reason
             FROM workspaces.signup_requests r
             JOIN workspaces.users u ON u.id = r.user_id
            WHERE r.user_id = $1"#,
        user as UserId
    )
    .fetch_optional(conn)
    .await?
    else {
        return Ok(None);
    };
    Ok(Some(SignupRequestView {
        user_id: row.user_id,
        display_name: row.display_name,
        email: row.email,
        status: RequestStatus::parse(&row.status)?,
        requested_at: row.requested_at,
        decided_at: row.decided_at,
        decided_by: row.decided_by,
        reason: row.reason,
    }))
}

/// Longest reason an administrator can give for a rejection.
pub const MAX_REASON: usize = 500;

/// Decide `user`'s signup request as `grant`'s administrator: approve it or
/// reject it (with an optional `reason`). Idempotent: deciding a request the
/// way it already stands changes nothing and still succeeds. The decision
/// goes to the security log, in the same transaction. A user with no
/// request is a 404.
pub async fn decide(
    state: &AppState,
    grant: &AdminGrant,
    user: UserId,
    to: RequestStatus,
    reason: Option<&str>,
) -> Result<SignupRequestView> {
    debug_assert!(
        to != RequestStatus::Pending,
        "a request is decided, not reopened"
    );
    let reason = reason.map(str::trim).filter(|reason| !reason.is_empty());
    if reason.is_some_and(|reason| reason.chars().count() > MAX_REASON) {
        return Err(Error::BadRequest(
            MESSAGES.reason("invalid_reason").arg("max", MAX_REASON),
        ));
    }
    let mut tx = begin_admin(&state.db, grant).await?;
    let current = sqlx::query_scalar!(
        "SELECT status FROM workspaces.signup_requests WHERE user_id = $1 FOR UPDATE",
        user as UserId
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(Error::NotFound)?;
    let before = RequestStatus::parse(&current)?;
    if before != to {
        sqlx::query!(
            "UPDATE workspaces.signup_requests
                SET status = $2, decided_at = now(), decided_by = $3, reason = $4
              WHERE user_id = $1",
            user as UserId,
            to.as_str(),
            grant.account(),
            reason,
        )
        .execute(&mut *tx)
        .await?;
        // The event is the requester's own (instance-level) one.
        set_user(&mut tx, user).await?;
        let kind = match to {
            RequestStatus::Rejected => SecurityKind::SignupRejected,
            RequestStatus::Approved | RequestStatus::Pending => SecurityKind::SignupApproved,
        };
        security::record(
            &mut tx,
            None,
            Some(user),
            kind,
            serde_json::json!({ "decided_by": grant.account() }),
        )
        .await?;
        let signup_request = tracked::signup_request(&mut tx, user).await?;
        let before = tracked::request_status(before);
        if to == RequestStatus::Rejected {
            state
                .tracker
                .track(
                    &mut tx,
                    &tracking::SignupRequestRejected {
                        signup_request,
                        before,
                    },
                )
                .await?;
        } else {
            state
                .tracker
                .track(
                    &mut tx,
                    &tracking::SignupRequestApproved {
                        signup_request,
                        before,
                    },
                )
                .await?;
        }
    }
    let view = request_view(&mut tx, user).await?.ok_or(Error::NotFound)?;
    tx.commit().await?;
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::{digest_of, matches_any, new_code};

    #[test]
    fn env_codes_match_exactly() {
        let codes = ["alpha", "beta"];
        assert!(matches_any("beta", &codes));
        assert!(matches_any(" alpha ", &codes));
        assert!(!matches_any("gamma", &codes));
        assert!(!matches_any("alph", &codes));
        assert!(!matches_any("", &[]));
    }

    #[test]
    fn minted_codes_are_readable_and_forgiving() {
        let (code, stored) = new_code().unwrap();
        assert_eq!(code.len(), 19, "{code}");
        assert_eq!(code.matches('-').count(), 3);
        assert!(code.chars().all(|c| c == '-' || c.is_ascii_alphanumeric()));
        assert!(!code.contains(['0', 'O', '1', 'I']));
        let lazy = code.to_lowercase().replace('-', " ");
        assert_eq!(digest_of(&format!("  {lazy} ")).0, stored);
        assert_ne!(new_code().unwrap().0, code);
    }
}
