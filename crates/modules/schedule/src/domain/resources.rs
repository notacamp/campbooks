//! The module's part of the access catalogue (ADR 0017): a calendar and its
//! events are their member's alone. Nobody else sees them (404): not a
//! workspace owner, not another member; a client token has no grant, as it
//! acts for no member.

use std::sync::Arc;

use campbooks_kernel::{
    RecordRef,
    authz::{
        Action, Facts, FactsFuture, Owner, OwnerFact, ResourceFacts, ResourceType, Resources,
        Tenancy,
    },
    changes::{ContainersFuture, Visibility, VisibilityFuture, VisibilityPort},
    tenancy::{Access, UserId},
};
use sqlx::{PgConnection, PgPool};

use crate::model::{CALENDAR_RECORD, CalendarId, EVENT_RECORD, EventId, MEMBER_RECORD};

/// What a member does with their own calendars and events.
const OWNER_ACTIONS: &[Action] = &[Action::Read, Action::Write, Action::Delete, Action::Create];

/// The module's resource types.
static TYPES: [ResourceType; 2] = [
    ResourceType {
        owner: Owner::Member,
        owner_actions: OWNER_ACTIONS,
        ..ResourceType::new(
            CALENDAR_RECORD,
            "A member's calendar: their Campbooks calendar, or one of a connected provider \
             account. Only the member sees it; other members, owners and client tokens do not.",
            Tenancy::Workspace,
        )
    },
    ResourceType {
        owner: Owner::Member,
        owner_actions: OWNER_ACTIONS,
        ..ResourceType::new(
            EVENT_RECORD,
            "An event on a member's calendar. Only the member sees it; other members, owners \
             and client tokens do not.",
            Tenancy::Workspace,
        )
    },
];

/// The module's resources, for its descriptor.
pub fn resources() -> Resources {
    Resources::new(&TYPES, Arc::new(CalendarFacts))
}

#[derive(Debug, Clone, Copy)]
struct CalendarFacts;

impl ResourceFacts for CalendarFacts {
    fn facts<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        _access: &'a Access,
        resource: &'a RecordRef,
    ) -> FactsFuture<'a> {
        Box::pin(async move {
            let owner: Option<UserId> = match resource.record_type.as_str() {
                CALENDAR_RECORD => match resource.id.parse::<CalendarId>() {
                    Ok(id) => {
                        sqlx::query_scalar("SELECT member_id FROM schedule.calendars WHERE id = $1")
                            .bind(id)
                            .fetch_optional(conn)
                            .await?
                    }
                    Err(_) => None,
                },
                EVENT_RECORD => match resource.id.parse::<EventId>() {
                    Ok(id) => {
                        sqlx::query_scalar(
                            "SELECT c.member_id FROM schedule.events e
                               JOIN schedule.calendars c ON c.id = e.calendar_id
                              WHERE e.id = $1",
                        )
                        .bind(id)
                        .fetch_optional(conn)
                        .await?
                    }
                    Err(_) => None,
                },
                _ => None,
            };
            Ok(owner.map(|member| Facts::owned_by(OwnerFact::User(member))))
        })
    }
}

/// The change feed's port for this module's container, for the server to
/// register: `schedule.member:<member>` holds what concerns one member's
/// calendars outside this module (their invitations, as attention items,
/// campbooks-cloud#360), and only that member sees inside.
pub fn visibility() -> VisibilityPort {
    Arc::new(MemberVisibility)
}

/// A member's calendars are seen by that member alone.
#[derive(Debug, Clone, Copy, Default)]
pub struct MemberVisibility;

impl Visibility for MemberVisibility {
    fn container_types(&self) -> &'static [&'static str] {
        &[MEMBER_RECORD]
    }

    fn container_visible<'a>(
        &'a self,
        _db: &'a PgPool,
        access: &'a Access,
        container: &'a RecordRef,
    ) -> VisibilityFuture<'a> {
        let visible = container.record_type == MEMBER_RECORD
            && access
                .user
                .is_some_and(|user| user.to_string() == container.id);
        Box::pin(async move { Ok(visible) })
    }

    fn visible_containers<'a>(
        &'a self,
        _db: &'a PgPool,
        access: &'a Access,
        container_type: &'a str,
    ) -> ContainersFuture<'a> {
        let listed: Vec<RecordRef> = if container_type == MEMBER_RECORD {
            access
                .user
                .map(|user| RecordRef::new(MEMBER_RECORD, &user))
                .into_iter()
                .collect()
        } else {
            Vec::new()
        };
        Box::pin(async move { Ok(listed) })
    }
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::{
        RecordRef,
        changes::Visibility as _,
        tenancy::{Access, UserId, WorkspaceId},
    };
    use sqlx::PgPool;

    use super::MemberVisibility;
    use crate::model::MEMBER_RECORD;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn only_the_member_sees_inside_their_container(db: PgPool) {
        let workspace = WorkspaceId::new();
        let ana = UserId::new();
        let access = |user| Access {
            workspace,
            user,
            role: None,
        };
        let theirs = RecordRef::new(MEMBER_RECORD, &ana);
        let port = MemberVisibility;
        assert!(
            port.container_visible(&db, &access(Some(ana)), &theirs)
                .await
                .unwrap()
        );
        assert!(
            !port
                .container_visible(&db, &access(Some(UserId::new())), &theirs)
                .await
                .unwrap()
        );
        assert!(
            !port
                .container_visible(&db, &access(None), &theirs)
                .await
                .unwrap()
        );
        let listed = port
            .visible_containers(&db, &access(Some(ana)), MEMBER_RECORD)
            .await
            .unwrap();
        assert_eq!(listed, vec![theirs]);
    }
}
