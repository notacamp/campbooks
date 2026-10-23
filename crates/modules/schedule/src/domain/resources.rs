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
    tenancy::{Access, UserId},
};
use sqlx::PgConnection;

use crate::model::{CALENDAR_RECORD, CalendarId, EVENT_RECORD, EventId};

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
