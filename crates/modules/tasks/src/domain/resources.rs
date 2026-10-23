//! The module's part of the access catalogue (ADR 0017), and the change
//! feed's `Visibility` port for its containers.
//!
//! A task is its assignee's: they read it and change it. The member who
//! handed it off still reads it (as "with <name>") and may take it back
//! while it is open, which the facts express by placing a handed-off task
//! in that member's own container, `tasks.assignee:<member>`, that only they
//! may see. Owners and admins of the workspace get nothing.

use std::sync::Arc;

use campbooks_kernel::{
    RecordRef,
    authz::{
        Action, Facts, FactsFuture, Grant, Grantee, Owner, OwnerFact, ResourceFacts, ResourceType,
        Resources, Tenancy,
    },
    changes::{ContainersFuture, Visibility, VisibilityFuture, VisibilityPort},
    tenancy::{Access, UserId},
};
use sqlx::{PgConnection, PgPool};

use super::{State, TaskId};

/// The record type of a task.
pub const TASK_RECORD: &str = "tasks.task";

/// The container type of a member's tasks: only that member sees inside.
pub const ASSIGNEE_RECORD: &str = "tasks.assignee";

/// The module's resource types.
static TYPES: [ResourceType; 1] = [ResourceType {
    owner: Owner::Member,
    owner_actions: &[Action::Read, Action::Write, Action::Share, Action::Create],
    grants: &[Grant::new(
        Grantee::ContainerViewers,
        &[Action::Read, Action::Share],
    )],
    ..ResourceType::new(
        TASK_RECORD,
        "Something a member has to do. Its assignee reads and changes it; the member who \
         handed it off reads it and takes it back while it is open. Nobody else sees it.",
        Tenancy::Workspace,
    )
}];

/// The module's resources, for its descriptor.
pub fn resources() -> Resources {
    Resources::new(&TYPES, Arc::new(TaskFacts))
}

#[derive(Debug, Clone, Copy)]
struct TaskFacts;

impl ResourceFacts for TaskFacts {
    fn facts<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        _access: &'a Access,
        resource: &'a RecordRef,
    ) -> FactsFuture<'a> {
        Box::pin(async move {
            if resource.record_type != TASK_RECORD {
                return Ok(None);
            }
            let Ok(id) = resource.id.parse::<TaskId>() else {
                return Ok(None);
            };
            let found: Option<(UserId, Option<UserId>, String)> =
                sqlx::query_as("SELECT assignee, handed_by, state FROM tasks.tasks WHERE id = $1")
                    .bind(id)
                    .fetch_optional(conn)
                    .await?;
            Ok(found.map(|(assignee, handed_by, state)| {
                let facts = Facts::owned_by(OwnerFact::User(assignee));
                match handed_by {
                    Some(by) if state == State::Open.as_str() => {
                        facts.within(RecordRef::new(ASSIGNEE_RECORD, &by), None)
                    }
                    _ => facts,
                }
            }))
        })
    }
}

/// The port, for the server to register.
pub fn visibility() -> VisibilityPort {
    Arc::new(AssigneeVisibility)
}

/// A member's tasks are seen by that member alone.
#[derive(Debug, Clone, Copy, Default)]
pub struct AssigneeVisibility;

impl Visibility for AssigneeVisibility {
    fn container_types(&self) -> &'static [&'static str] {
        &[ASSIGNEE_RECORD]
    }

    fn container_visible<'a>(
        &'a self,
        _db: &'a PgPool,
        access: &'a Access,
        container: &'a RecordRef,
    ) -> VisibilityFuture<'a> {
        let visible = container.record_type == ASSIGNEE_RECORD
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
        let listed: Vec<RecordRef> = if container_type == ASSIGNEE_RECORD {
            access
                .user
                .map(|user| RecordRef::new(ASSIGNEE_RECORD, &user))
                .into_iter()
                .collect()
        } else {
            Vec::new()
        };
        Box::pin(async move { Ok(listed) })
    }
}
