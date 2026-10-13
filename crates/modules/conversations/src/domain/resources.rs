//! The module's part of the access catalogue (ADR 0017).
//!
//! An **inbox** is owned by the member whose connection feeds it and shared
//! with its members by role: `reader` reads, `sender` also writes and sends,
//! `manager` also shares. Everything inside (conversations, messages,
//! attachments, decisions, labels, corrections) follows the inbox. A
//! **draft** is private to its author, who reads and deletes it while they
//! read the inbox and writes and sends it while they may send from it.
//! Client tokens bound to the workspace get an explicit grant: read and
//! write in every inbox, their own drafts. Workspace owners and admins get
//! nothing here: they never read an inbox they are not a member of. An
//! inbox kept after its connection was removed is frozen read-only.

use std::sync::Arc;

use campbooks_kernel::{
    Error, RecordRef, Result,
    authz::{
        self, Action, Facts, FactsFuture, Grant, Grantee, Owner, OwnerFact, ResourceFacts,
        ResourceType, Resources, Tenancy,
    },
    tenancy::{Access, UserId},
};
use sqlx::PgConnection;

use crate::{
    events::{CONVERSATION_RECORD, DRAFT_RECORD, INBOX_RECORD, MESSAGE_RECORD},
    features::apply_changes::ATTACHMENT_RECORD,
    model::InboxId,
};

/// The record type of a decide-pass answer.
pub const DECISION_RECORD: &str = "conversations.decision";
/// The record type of a label.
pub const LABEL_RECORD: &str = "conversations.label";
/// The record type of a member's correction.
pub const CORRECTION_RECORD: &str = "conversations.correction";

use Action::{Delete, Read, Send, Share, Write};

/// Read for readers and above, and for client tokens: what lives inside an
/// inbox and is only read.
const READ_INSIDE: &[Grant] = &[
    Grant::new(Grantee::ContainerRole("reader"), &[Read]),
    Grant::new(Grantee::Clients, &[Read]),
];

/// Read for readers, write for senders and above, both for client tokens.
const WRITE_INSIDE: &[Grant] = &[
    Grant::new(Grantee::ContainerRole("reader"), &[Read]),
    Grant::new(Grantee::ContainerRole("sender"), &[Read, Write]),
    Grant::new(Grantee::Clients, &[Read, Write]),
];

const fn inside(
    name: &'static str,
    description: &'static str,
    grants: &'static [Grant],
) -> ResourceType {
    ResourceType {
        container: Some(INBOX_RECORD),
        grants,
        ..ResourceType::new(name, description, Tenancy::Workspace)
    }
}

/// The module's resource types.
static TYPES: [ResourceType; 8] = [
    ResourceType {
        owner: Owner::Member,
        owner_actions: &[Read],
        container: Some(INBOX_RECORD),
        roles: &["reader", "sender", "manager"],
        grants: &[
            Grant::new(Grantee::ContainerRole("reader"), &[Read]),
            Grant::new(Grantee::ContainerRole("sender"), &[Read, Send]),
            Grant::new(Grantee::ContainerRole("manager"), &[Read, Send, Share]),
            Grant::new(Grantee::Clients, &[Read, Send]),
        ],
        ..ResourceType::new(
            INBOX_RECORD,
            "An address the workspace receives and sends on, owned by the member whose \
             connection feeds it and shared with its members by role (`reader` < `sender` < \
             `manager`). Workspace owners and admins get nothing without a membership. Kept \
             after its connection is removed, it is read-only.",
            Tenancy::Workspace,
        )
    },
    inside(
        CONVERSATION_RECORD,
        "A conversation in an inbox: its readers read it, senders mark, label, archive and \
         move it.",
        WRITE_INSIDE,
    ),
    inside(
        MESSAGE_RECORD,
        "A message in an inbox, its body and headers: read by the inbox's readers.",
        READ_INSIDE,
    ),
    inside(
        ATTACHMENT_RECORD,
        "A file a message carries: read by the inbox's readers.",
        READ_INSIDE,
    ),
    inside(
        DECISION_RECORD,
        "What the decide pass answered about a message, and why: read by the inbox's readers.",
        READ_INSIDE,
    ),
    inside(
        LABEL_RECORD,
        "A label of an inbox: its readers see it, senders put it on and take it off.",
        WRITE_INSIDE,
    ),
    inside(
        CORRECTION_RECORD,
        "A member moving a conversation to another stream, kept as an example: made by the \
         inbox's senders.",
        WRITE_INSIDE,
    ),
    ResourceType {
        owner: Owner::Member,
        container: Some(INBOX_RECORD),
        grants: &[
            Grant::new(Grantee::OwnerWithContainerRole("reader"), &[Read, Delete]),
            Grant::new(
                Grantee::OwnerWithContainerRole("sender"),
                &[Read, Write, Delete, Send],
            ),
            Grant::new(Grantee::OwningClients, &[Read, Write, Delete, Send]),
        ],
        ..ResourceType::new(
            DRAFT_RECORD,
            "A message being written, private to its author (a member, or the workspace's \
             client tokens for one a client wrote): read and deleted while they read its inbox, \
             written and sent while they may send from it. Nobody else sees it.",
            Tenancy::Workspace,
        )
    },
];

/// The module's resources, for its descriptor.
pub fn resources() -> Resources {
    Resources::new(&TYPES, Arc::new(ConversationFacts))
}

/// `inbox`, as a resource.
pub fn inbox(inbox: InboxId) -> RecordRef {
    RecordRef::new(INBOX_RECORD, &inbox)
}

/// Ask the catalogue whether `access` may take `action` on `resource`:
/// 404 when they may not see it, 403 `inbox_read_only` when they may see
/// it but not do that (a reader writing, a kept inbox).
pub async fn authorize(
    state: &campbooks_kernel::AppState,
    conn: &mut PgConnection,
    access: &Access,
    action: Action,
    resource: &RecordRef,
) -> Result<()> {
    authz::require(state, conn, access, action, resource, || {
        Error::Denied(crate::MESSAGES.reason("inbox_read_only"))
    })
    .await
}

/// [`authorize`] for a type before one is named (lists).
pub fn authorize_type(
    state: &campbooks_kernel::AppState,
    access: &Access,
    action: Action,
    resource_type: &str,
) -> Result<()> {
    authz::require_type(state, access, action, resource_type, || {
        Error::Denied(crate::MESSAGES.reason("inbox_read_only"))
    })
}

/// The facts of the module's resources: which inbox a record lives in, the
/// caller's role there, and whether it is kept read-only.
#[derive(Debug, Clone, Copy)]
struct ConversationFacts;

impl ResourceFacts for ConversationFacts {
    fn facts<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        access: &'a Access,
        resource: &'a RecordRef,
    ) -> FactsFuture<'a> {
        Box::pin(async move {
            // Every id of the module is a UUID; the inbox id type parses one.
            let Ok(id) = resource.id.parse::<InboxId>() else {
                return Ok(None);
            };
            let id = *id.as_uuid();
            // The inbox the record lives in, and its owner when it has one
            // of its own (an inbox, a draft).
            let located: Option<(InboxId, OwnerFact)> = match resource.record_type.as_str() {
                INBOX_RECORD => sqlx::query!(
                    r#"SELECT id AS "id: InboxId", owner FROM conversations.inboxes WHERE id = $1"#,
                    id
                )
                .fetch_optional(&mut *conn)
                .await?
                .map(|row| (row.id, row.owner.map_or(OwnerFact::Nobody, |o| OwnerFact::User(UserId::from_uuid(o))))),
                DRAFT_RECORD => sqlx::query!(
                    r#"SELECT inbox_id AS "inbox_id: InboxId", author FROM conversations.drafts WHERE id = $1"#,
                    id
                )
                .fetch_optional(&mut *conn)
                .await?
                .map(|row| {
                    let owner = row
                        .author
                        .map_or(OwnerFact::Clients, |a| OwnerFact::User(UserId::from_uuid(a)));
                    (row.inbox_id, owner)
                }),
                other => {
                    let inbox = match other {
                        CONVERSATION_RECORD => sqlx::query_scalar!(
                            r#"SELECT inbox_id AS "inbox_id: InboxId" FROM conversations.conversations WHERE id = $1"#,
                            id
                        )
                        .fetch_optional(&mut *conn)
                        .await?,
                        MESSAGE_RECORD => sqlx::query_scalar!(
                            r#"SELECT inbox_id AS "inbox_id: InboxId" FROM conversations.messages WHERE id = $1"#,
                            id
                        )
                        .fetch_optional(&mut *conn)
                        .await?,
                        ATTACHMENT_RECORD => sqlx::query_scalar!(
                            r#"SELECT m.inbox_id AS "inbox_id!: InboxId" FROM conversations.attachments a
                                 JOIN conversations.messages m ON m.id = a.message_id
                                WHERE a.id = $1"#,
                            id
                        )
                        .fetch_optional(&mut *conn)
                        .await?,
                        DECISION_RECORD => sqlx::query_scalar!(
                            r#"SELECT m.inbox_id AS "inbox_id!: InboxId" FROM conversations.decisions d
                                 JOIN conversations.messages m ON m.id = d.message_id
                                WHERE d.id = $1"#,
                            id
                        )
                        .fetch_optional(&mut *conn)
                        .await?,
                        LABEL_RECORD => sqlx::query_scalar!(
                            r#"SELECT inbox_id AS "inbox_id: InboxId" FROM conversations.labels WHERE id = $1"#,
                            id
                        )
                        .fetch_optional(&mut *conn)
                        .await?,
                        CORRECTION_RECORD => sqlx::query_scalar!(
                            r#"SELECT c.inbox_id AS "inbox_id!: InboxId" FROM conversations.corrections r
                                 JOIN conversations.conversations c ON c.id = r.conversation_id
                                WHERE r.id = $1"#,
                            id
                        )
                        .fetch_optional(&mut *conn)
                        .await?,
                        _ => None,
                    };
                    inbox.map(|inbox| (inbox, OwnerFact::Nobody))
                }
            };
            let Some((inbox, owner)) = located else {
                return Ok(None);
            };
            let found = sqlx::query!(
                r#"SELECT i.detached_at IS NOT NULL AS "frozen!",
                          (SELECT m.role FROM conversations.inbox_members m
                            WHERE m.inbox_id = i.id AND m.user_id = $2) AS role
                     FROM conversations.inboxes i WHERE i.id = $1"#,
                inbox as InboxId,
                access.user.map(|user| *user.as_uuid()),
            )
            .fetch_optional(&mut *conn)
            .await?;
            Ok(found.map(|row| {
                Facts::owned_by(owner)
                    .within(RecordRef::new(INBOX_RECORD, &inbox), row.role)
                    .frozen(row.frozen)
            }))
        })
    }
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::{
        authz::{Catalogue, Decision, decide},
        tenancy::{Role, WorkspaceId},
    };

    use super::*;

    fn member(user: UserId, role: Role) -> Access {
        Access {
            workspace: WorkspaceId::new(),
            user: Some(user),
            role: Some(role),
        }
    }

    fn facts(owner: OwnerFact, role: Option<&str>) -> Facts {
        Facts::owned_by(owner).within(
            RecordRef::new(INBOX_RECORD, &InboxId::new()),
            role.map(ToOwned::to_owned),
        )
    }

    #[test]
    fn the_catalogue_is_well_formed_and_every_type_is_known() {
        let catalogue = Catalogue::new([&resources()]).unwrap();
        for declared in &TYPES {
            assert!(catalogue.get(declared.name).is_some());
        }
    }

    #[test]
    fn workspace_owners_never_read_an_inbox_they_are_not_in() {
        let catalogue = Catalogue::new([&resources()]).unwrap();
        let owner = member(UserId::new(), Role::Owner);
        for name in [
            INBOX_RECORD,
            CONVERSATION_RECORD,
            MESSAGE_RECORD,
            DRAFT_RECORD,
        ] {
            let decision = decide(
                &catalogue,
                &owner,
                Read,
                name,
                Some(&facts(OwnerFact::Nobody, None)),
            );
            assert_eq!(decision, Decision::NotFound, "{name}");
        }
    }

    #[test]
    fn drafts_are_their_authors_alone() {
        let catalogue = Catalogue::new([&resources()]).unwrap();
        let author = UserId::new();
        let draft = |role| facts(OwnerFact::User(author), role);
        let check = |access: &Access, action, role| {
            decide(&catalogue, access, action, DRAFT_RECORD, Some(&draft(role)))
        };
        let me = member(author, Role::Member);
        assert_eq!(check(&me, Send, Some("sender")), Decision::Allow);
        assert_eq!(check(&me, Send, Some("reader")), Decision::Forbidden);
        assert_eq!(check(&me, Read, Some("reader")), Decision::Allow);
        assert_eq!(check(&me, Read, None), Decision::NotFound);
        let colleague = member(UserId::new(), Role::Admin);
        assert_eq!(check(&colleague, Read, Some("manager")), Decision::NotFound);
        let client = Access {
            workspace: WorkspaceId::new(),
            user: None,
            role: None,
        };
        assert_eq!(check(&client, Read, None), Decision::NotFound);
        let written = facts(OwnerFact::Clients, None);
        assert_eq!(
            decide(&catalogue, &client, Send, DRAFT_RECORD, Some(&written)),
            Decision::Allow
        );
    }

    #[test]
    fn a_kept_inbox_is_read_only() {
        let catalogue = Catalogue::new([&resources()]).unwrap();
        let user = UserId::new();
        let kept = facts(OwnerFact::Nobody, Some("manager")).frozen(true);
        let me = member(user, Role::Member);
        assert_eq!(
            decide(&catalogue, &me, Read, CONVERSATION_RECORD, Some(&kept)),
            Decision::Allow
        );
        assert_eq!(
            decide(&catalogue, &me, Write, CONVERSATION_RECORD, Some(&kept)),
            Decision::Forbidden
        );
        assert_eq!(
            decide(&catalogue, &me, Send, INBOX_RECORD, Some(&kept)),
            Decision::Forbidden
        );
    }
}
