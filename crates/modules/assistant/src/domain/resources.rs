//! The module's part of the access catalogue (ADR 0017): a member's profile
//! and their chats with Scout are theirs alone. No workspace role is granted
//! anything on them (owners and admins included), and no client token gets
//! them.

use std::sync::Arc;

use campbooks_kernel::{
    AppState, Error, RecordRef, Result,
    authz::{
        self, Action, Facts, FactsFuture, Owner, OwnerFact, ResourceFacts, ResourceType, Resources,
        Tenancy,
    },
    tenancy::{Access, UserId},
};
use sqlx::PgConnection;

/// The record type of a member's profile; its id is the member's user id.
pub const PROFILE_RECORD: &str = "assistant.profile";

/// The record type of a member's chat with Scout (campbooks-cloud#305).
pub const CHAT_RECORD: &str = "assistant.chat";

/// The module's resource types.
static TYPES: [ResourceType; 2] = [
    ResourceType {
        owner: Owner::Member,
        owner_actions: &[Action::Read, Action::Write],
        ..ResourceType::new(
            PROFILE_RECORD,
            "What a member told Scout about how to act, and compiling their own sentences \
             into rules to check. Private to the member: no workspace role, owners and \
             admins included, and no client token gets it.",
            Tenancy::Workspace,
        )
    },
    ResourceType {
        owner: Owner::Member,
        owner_actions: &[Action::Read, Action::Write, Action::Delete, Action::Create],
        ..ResourceType::new(
            CHAT_RECORD,
            "A member's chat with Scout about something they are looking at: their \
             questions and Scout's answers. Private to the member: no workspace role, \
             owners and admins included, and no client token gets it.",
            Tenancy::Workspace,
        )
    },
];

/// The module's resources, for its descriptor.
pub fn resources() -> Resources {
    Resources::new(&TYPES, Arc::new(AssistantFacts))
}

/// Chat `id`, as a resource.
pub fn chat(id: &impl ToString) -> RecordRef {
    RecordRef::new(CHAT_RECORD, id)
}

/// The caller's own profile, as a resource: a profile is addressed by its
/// member, and a caller only ever names their own.
pub fn own_profile(user: UserId) -> RecordRef {
    RecordRef::new(PROFILE_RECORD, &user)
}

/// [`authz::require`] on the caller's own profile: a member may read and
/// write theirs; a client token (no member) is refused with
/// `profile_needs_member`.
pub async fn require_own(
    state: &AppState,
    conn: &mut PgConnection,
    access: &Access,
    action: Action,
) -> Result<UserId> {
    let user = access.user.ok_or_else(needs_member)?;
    authz::require(
        state,
        conn,
        access,
        action,
        &own_profile(user),
        needs_member,
    )
    .await?;
    Ok(user)
}

fn needs_member() -> Error {
    Error::Denied(crate::MESSAGES.reason("profile_needs_member"))
}

/// A profile exists for every member (the defaults until they answer), and
/// is owned by that member; a chat is owned by the member it was started by.
#[derive(Debug, Clone, Copy)]
struct AssistantFacts;

impl ResourceFacts for AssistantFacts {
    fn facts<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        access: &'a Access,
        resource: &'a RecordRef,
    ) -> FactsFuture<'a> {
        Box::pin(async move {
            match resource.record_type.as_str() {
                PROFILE_RECORD => {
                    let Ok(member) = resource.id.parse::<UserId>() else {
                        return Ok(None);
                    };
                    // A member's profile exists while they are a member,
                    // which the workspace extractor checked for the caller;
                    // another member's is never named by an operation.
                    Ok((access.user == Some(member))
                        .then(|| Facts::owned_by(OwnerFact::User(member))))
                }
                CHAT_RECORD => {
                    let Ok(id) = resource.id.parse::<sqlx::types::Uuid>() else {
                        return Ok(None);
                    };
                    // In the caller's workspace transaction: row-level
                    // security keeps other workspaces' chats out.
                    let owner: Option<sqlx::types::Uuid> =
                        sqlx::query_scalar("SELECT user_id FROM assistant.chats WHERE id = $1")
                            .bind(id)
                            .fetch_optional(conn)
                            .await?;
                    Ok(owner.map(|user| Facts::owned_by(OwnerFact::User(UserId::from_uuid(user)))))
                }
                _ => Ok(None),
            }
        })
    }
}
