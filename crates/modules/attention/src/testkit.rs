//! Test fixtures: the module over a database whose connections run as the
//! runtime role (row-level security applies), a test identity provider,
//! stand-ins for the membership and container-visibility ports, and a way to
//! publish `conversations.need_changed` as the conversations module would.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    body::Body,
    http::{Method, Request, header},
};
use campbooks_kernel::{
    AppState, Error, RecordRef,
    auth::{Principal, PrincipalKind},
    changes::{Visibility, VisibilityFuture},
    db::BeginFor as _,
    jobs::{Registry, Worker, WorkerStats},
    tenancy::{Access, Membership, MembershipFuture, Role, UserId, WorkspaceId},
    testing::{self, TestIdp, TestResponse},
};
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;

use crate::{
    domain::{Reason, Trust},
    events::{
        ConversationNeedChanged, InvitationChanged, InvitationFacts, NeedFacts, TaskChanged,
        TaskState, TaskVerb,
    },
};

/// Every scope of this module.
pub const ALL_SCOPES: &str = "attention:read attention:write";

#[derive(Debug, Default)]
struct TestMembership {
    members: Mutex<HashMap<(String, WorkspaceId), UserId>>,
}

impl Membership for TestMembership {
    fn resolve<'a>(
        &'a self,
        _db: &'a PgPool,
        principal: &'a Principal,
        workspace: WorkspaceId,
    ) -> MembershipFuture<'a> {
        let access = match principal.kind {
            PrincipalKind::Client => Some(Access {
                workspace,
                user: None,
                role: None,
            }),
            PrincipalKind::User => self
                .members
                .lock()
                .ok()
                .and_then(|m| m.get(&(principal.subject.clone(), workspace)).copied())
                .map(|user| Access {
                    workspace,
                    user: Some(user),
                    role: Some(Role::Member),
                }),
        };
        Box::pin(async move { Ok::<_, Error>(access) })
    }
}

/// Inboxes and who may see them, as the conversations module would answer.
#[derive(Debug, Default)]
struct TestInboxes {
    members: Mutex<HashSet<(String, UserId)>>,
}

impl Visibility for TestInboxes {
    fn container_visible<'a>(
        &'a self,
        _db: &'a PgPool,
        access: &'a Access,
        container: &'a RecordRef,
    ) -> VisibilityFuture<'a> {
        let visible = match container.record_type.as_str() {
            "conversations.inbox" => match access.user {
                None => true,
                Some(user) => self
                    .members
                    .lock()
                    .is_ok_and(|m| m.contains(&(container.id.clone(), user))),
            },
            // A member's tasks, and their invitations, are that member's alone.
            "tasks.assignee" | "schedule.member" => access
                .user
                .is_some_and(|user| user.to_string() == container.id),
            _ => false,
        };
        Box::pin(async move { Ok(visible) })
    }
}

/// The module over a fresh database.
pub struct Kit {
    /// The test database as its owner (no row-level security).
    pub db: PgPool,
    /// The state jobs and handlers run with (runtime role).
    pub state: AppState,
    app: Router,
    idp: TestIdp,
    membership: Arc<TestMembership>,
    inboxes: Arc<TestInboxes>,
}

/// A need as `conversations.need_changed` would carry it.
#[derive(Clone)]
pub struct Need {
    pub conversation: String,
    pub inbox: String,
    pub message: String,
    pub revision: i64,
    pub reason: Reason,
    pub due_on: Option<NaiveDate>,
    pub sent_at: DateTime<Utc>,
    /// The conversation's newest inbound message, when not `message`.
    pub latest: Option<String>,
    /// When what it asks stops being possible.
    pub lapses_at: Option<DateTime<Utc>>,
    /// Whether it is the warning about suspicious mail.
    pub warning: bool,
    /// Whether a person sent its message; `None` when unknown.
    pub from_person: Option<bool>,
}

/// A task as `tasks.task_changed` would carry it.
#[derive(Clone)]
pub struct Task {
    pub id: String,
    pub revision: i64,
    pub state: TaskState,
    pub verb: TaskVerb,
    pub due_on: Option<NaiveDate>,
    /// The day it is planned for (its plan is then `date`).
    pub plan_on: Option<NaiveDate>,
    pub later_until: Option<DateTime<Utc>>,
    pub assignee: UserId,
    /// The conversation it came from.
    pub source: Option<String>,
}

impl Task {
    /// An open `do` task numbered `revision`, with nothing else.
    pub fn open(id: &str, revision: i64, assignee: UserId) -> Self {
        Self {
            id: id.into(),
            revision,
            state: TaskState::Open,
            verb: TaskVerb::Do,
            due_on: None,
            plan_on: None,
            later_until: None,
            assignee,
            source: None,
        }
    }
}

/// An invitation as `schedule.invitation_changed` would carry it.
#[derive(Clone)]
pub struct Invitation {
    pub event: String,
    pub revision: i64,
    pub owner: UserId,
    /// The day its event starts; `None` once it no longer awaits an answer.
    pub on: Option<NaiveDate>,
    pub lapses_at: DateTime<Utc>,
    pub from_person: Option<bool>,
    /// Why it no longer awaits one, once it does not.
    pub closed_because: Option<String>,
}

impl Invitation {
    /// An open invitation numbered `revision` for an event on `on`, from a
    /// person, lapsing a year from now.
    pub fn open(event: &str, revision: i64, owner: UserId, on: NaiveDate) -> Self {
        Self {
            event: event.into(),
            revision,
            owner,
            on: Some(on),
            lapses_at: Utc::now() + chrono::TimeDelta::days(365),
            from_person: Some(true),
            closed_because: None,
        }
    }

    /// The same invitation closed `because`, numbered `revision`.
    #[must_use]
    pub fn closed(&self, revision: i64, because: &str) -> Self {
        Self {
            revision,
            on: None,
            closed_because: Some(because.into()),
            ..self.clone()
        }
    }
}

impl Kit {
    /// Publish `schedule.invitation_changed` for `invitation` and run the
    /// jobs it causes.
    pub async fn invitation(&self, workspace: WorkspaceId, invitation: &Invitation) -> WorkerStats {
        let event = InvitationChanged {
            workspace: workspace.to_string(),
            event: invitation.event.clone(),
            owner: invitation.owner,
            revision: invitation.revision,
            invitation: invitation.on.map(|on| InvitationFacts {
                on,
                lapses_at: invitation.lapses_at,
                from_person: invitation.from_person,
            }),
            closed_because: invitation.closed_because.clone(),
        };
        let mut tx = self.state.db.begin_for(workspace).await.unwrap();
        self.state.events.publish(&mut tx, &event).await.unwrap();
        tx.commit().await.unwrap();
        self.run_jobs().await
    }

    /// Publish `tasks.task_changed` for `task` and run the jobs it causes.
    pub async fn task(&self, workspace: WorkspaceId, task: &Task) -> WorkerStats {
        let event = TaskChanged {
            workspace: workspace.to_string(),
            task: task.id.clone(),
            revision: task.revision,
            state: task.state,
            verb: task.verb,
            due_on: task.due_on,
            plan: if task.later_until.is_some() {
                "later"
            } else if task.plan_on.is_some() {
                "date"
            } else {
                "none"
            }
            .into(),
            plan_on: task.plan_on,
            later_until: task.later_until,
            assignee: task.assignee,
            source: task
                .source
                .as_ref()
                .map(|id| RecordRef::new("conversations.conversation", id)),
        };
        let mut tx = self.state.db.begin_for(workspace).await.unwrap();
        self.state.events.publish(&mut tx, &event).await.unwrap();
        tx.commit().await.unwrap();
        self.run_jobs().await
    }

    /// The module over `db`.
    pub async fn new(db: PgPool) -> Self {
        let idp = TestIdp::start().await;
        let pool = testing::app_role_pool(&db).await;
        let membership = Arc::new(TestMembership::default());
        let inboxes = Arc::new(TestInboxes::default());
        let state = idp
            .state(pool)
            .with_membership(membership.clone())
            .with_visibility(inboxes.clone())
            .with_registry(Registry::build(&[crate::module()]).unwrap());
        let app = testing::app(crate::module(), state.clone());
        Self {
            db,
            state,
            app,
            idp,
            membership,
            inboxes,
        }
    }

    /// The same kit, writing tracked events to the outbox (ADR 0020), read
    /// back with [`testing::tracked_events`].
    #[must_use]
    pub fn tracking(mut self) -> Self {
        self.state = testing::tracking_outbox(self.state);
        self.app = testing::app(crate::module(), self.state.clone());
        self
    }

    /// A user `sub`, member of `workspace`, with every scope of the module.
    pub fn member(&self, sub: &str, workspace: WorkspaceId) -> (String, UserId) {
        let user = UserId::new();
        self.membership
            .members
            .lock()
            .unwrap()
            .insert((sub.to_owned(), workspace), user);
        let mut claims = self.idp.claims(ALL_SCOPES);
        claims["sub"] = sub.into();
        (self.idp.token(&claims), user)
    }

    /// A client token bound to `workspace`.
    pub fn client(&self, workspace: WorkspaceId, scopes: &str) -> String {
        let mut claims = self.idp.claims(scopes);
        let object = claims.as_object_mut().unwrap();
        object.remove("sub");
        object.insert("azp".into(), "acme-sync".into());
        object.insert("workspace_id".into(), workspace.to_string().into());
        self.idp.token(&claims)
    }

    /// Let `user` see inbox `inbox`.
    pub fn let_see(&self, inbox: &str, user: UserId) {
        self.inboxes
            .members
            .lock()
            .unwrap()
            .insert((inbox.to_owned(), user));
    }

    /// Publish `conversations.need_changed` for `need` (open) and run the
    /// jobs it causes.
    pub async fn need(&self, workspace: WorkspaceId, need: &Need) -> WorkerStats {
        self.publish(
            workspace,
            &ConversationNeedChanged {
                workspace: workspace.to_string(),
                inbox: need.inbox.clone(),
                conversation: need.conversation.clone(),
                message: need.message.clone(),
                revision: need.revision,
                sent_at: need.sent_at,
                need: Some(NeedFacts {
                    reason: need.reason,
                    trust: Trust::Sure,
                    due_on: need.due_on,
                    lapses_at: need.lapses_at,
                    urgent: false,
                    warning: need.warning,
                    from_person: need.from_person,
                }),
                cleared_because: None,
                cleared_by: None,
                latest_message: need.latest.clone(),
            },
        )
        .await
    }

    /// Publish a cleared `conversations.need_changed`.
    pub async fn cleared(
        &self,
        workspace: WorkspaceId,
        need: &Need,
        cause: &str,
        by: Option<UserId>,
    ) -> WorkerStats {
        self.publish(
            workspace,
            &ConversationNeedChanged {
                workspace: workspace.to_string(),
                inbox: need.inbox.clone(),
                conversation: need.conversation.clone(),
                message: need.message.clone(),
                revision: need.revision,
                sent_at: need.sent_at,
                need: None,
                cleared_because: Some(cause.to_owned()),
                cleared_by: by,
                latest_message: need.latest.clone(),
            },
        )
        .await
    }

    async fn publish(
        &self,
        workspace: WorkspaceId,
        event: &ConversationNeedChanged,
    ) -> WorkerStats {
        let mut tx = self.state.db.begin_for(workspace).await.unwrap();
        self.state.events.publish(&mut tx, event).await.unwrap();
        tx.commit().await.unwrap();
        self.run_jobs().await
    }

    /// Run every pending job until none is left.
    pub async fn run_jobs(&self) -> WorkerStats {
        let stats = Worker::new(self.state.clone()).run_until_idle().await;
        assert_eq!(stats.failed, 0, "{stats:?}");
        stats
    }

    /// Events of type `name` in the log, oldest first, as JSON.
    pub async fn events(&self, name: &str) -> Vec<serde_json::Value> {
        sqlx::query_scalar("SELECT payload FROM kernel.events WHERE name = $1 ORDER BY id")
            .bind(name)
            .fetch_all(&self.db)
            .await
            .unwrap()
    }

    /// `GET uri` with `token`.
    pub async fn get(&self, token: &str, uri: &str) -> TestResponse {
        self.call(Method::GET, token, uri, None).await
    }

    /// `POST uri` with a JSON body (or none).
    pub async fn post(
        &self,
        token: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> TestResponse {
        self.call(Method::POST, token, uri, body).await
    }

    async fn call(
        &self,
        method: Method,
        token: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> TestResponse {
        let mut request = Request::builder().method(method).uri(uri);
        if !token.is_empty() {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        };
        testing::send(&self.app, request.unwrap()).await
    }
}
