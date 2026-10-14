//! The access matrix (ADR 0017, campbooks-cloud#117), generated from the
//! catalogues so a new resource type or operation is covered automatically,
//! or fails here until it says how to seed it.
//!
//! - **Resources × principals × actions**: for every workspace resource type
//!   of the access catalogue, a seeded instance in workspace A is asked about
//!   by each kind of caller through `authz::authorize`, with the owning
//!   module's real facts; the answer must be what the catalogue declares for
//!   that caller's true relation to the instance (owner, container role,
//!   workspace role, client token), and nothing for a member of workspace B.
//! - **Operations × callers**: every bearer operation of `openapi.json` is
//!   called by an anonymous caller (401), a signed-in person in no
//!   workspace, a member of another workspace and an instance administrator
//!   (the same 404, byte for byte, as for a workspace that does not exist),
//!   and by each kind of member and a client token of the workspace: 404
//!   when the catalogue says they may not see the record, 403 when they may
//!   see it but not act, and otherwise an answer past the access checks.
//!   Lists answered to a member who sees no inbox carry none of its ids.

use std::collections::{BTreeMap, BTreeSet};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use campbooks_conversations::{dev, model::InboxRole};
use campbooks_kernel::{
    AppState, RecordRef,
    authz::{self, Action, Decision, Facts, OwnerFact, Tenancy},
    blobs::BlobStore,
    config::BlobsConfig,
    db::BeginFor as _,
    jobs::Worker,
    operations,
    tenancy::{Access, Role, UserId, WorkspaceId},
    testing::{self, TestIdp},
};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt as _;
use uuid::Uuid;

const EN: &str = include_str!("../../modules/conversations/fixtures/en.json");

/// Workspace resource types not answered per instance yet: search results
/// and the change feed are lists filtered at query time (tested in their
/// own suites); workspace clients move onto per-instance facts with the
/// identity module (campbooks-cloud#117, PR B) and are owner/admin-only by
/// type meanwhile. A new type must be seeded below, or listed here.
const NOT_PER_INSTANCE: [&str; 3] = ["changes.feed", "identity.client", "search.result"];

/// Every scope of the catalogue, so only access decides.
fn all_scopes() -> String {
    crate::scope_catalogue()
        .iter()
        .map(|scope| scope.name)
        .chain(["admin:access"])
        .collect::<Vec<_>>()
        .join(" ")
}

/// Who calls, in workspace A unless said otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Who {
    /// Workspace A's owner, in no inbox.
    Owner,
    /// An admin of A, in no inbox.
    Admin,
    /// A member of A with no inbox.
    Member,
    /// A member reading the inbox.
    Reader,
    /// A member sending from the inbox; the draft's author.
    Sender,
    /// A member who connected the mailbox (the inbox's owner and manager).
    Connector,
    /// A client token bound to A.
    Client,
}

const MEMBERS: [Who; 7] = [
    Who::Owner,
    Who::Admin,
    Who::Member,
    Who::Reader,
    Who::Sender,
    Who::Connector,
    Who::Client,
];

impl Who {
    fn subject(self) -> &'static str {
        match self {
            Self::Owner => "wes",
            Self::Admin => "adi",
            Self::Member => "mo",
            Self::Reader => "rea",
            Self::Sender => "sen",
            Self::Connector => "ana",
            Self::Client => "acme-sync",
        }
    }

    fn role(self) -> Option<Role> {
        match self {
            Self::Owner => Some(Role::Owner),
            Self::Admin => Some(Role::Admin),
            Self::Client => None,
            _ => Some(Role::Member),
        }
    }

    fn inbox_role(self) -> Option<&'static str> {
        match self {
            Self::Reader => Some("reader"),
            Self::Sender => Some("sender"),
            Self::Connector => Some("manager"),
            _ => None,
        }
    }
}

/// Two workspaces with every kind of record, and the app.
struct World {
    db: PgPool,
    app: Router,
    state: AppState,
    idp: TestIdp,
    a: WorkspaceId,
    b: WorkspaceId,
    users: BTreeMap<Who, UserId>,
    /// One instance of each workspace resource type in A.
    instances: BTreeMap<&'static str, String>,
    /// Every id of the inbox's records, for leak checks.
    inbox_ids: Vec<String>,
    /// This seed's suffix of every subject.
    run: String,
}

async fn user(db: &PgPool, sub: &str) -> UserId {
    let user = UserId::new();
    sqlx::query("INSERT INTO workspaces.users (id, idp_subject) VALUES ($1, $2)")
        .bind(user.as_uuid())
        .bind(sub)
        .execute(db)
        .await
        .unwrap();
    user
}

async fn workspace(db: &PgPool, name: &str) -> WorkspaceId {
    let id = WorkspaceId::new();
    sqlx::query(
        "INSERT INTO workspaces.workspaces (id, name, kind, accent) VALUES ($1, $2, 'business', 'business')",
    )
    .bind(id.as_uuid())
    .bind(name)
    .execute(db)
    .await
    .unwrap();
    id
}

async fn join(db: &PgPool, workspace: WorkspaceId, user: UserId, role: Role) {
    sqlx::query("INSERT INTO workspaces.members (workspace_id, user_id, role) VALUES ($1, $2, $3)")
        .bind(workspace.as_uuid())
        .bind(user.as_uuid())
        .bind(role.as_str())
        .execute(db)
        .await
        .unwrap();
}

async fn scalar(db: &PgPool, sql: &'static str, bind: Uuid) -> String {
    sqlx::query_scalar(sql)
        .bind(bind)
        .fetch_one(db)
        .await
        .unwrap_or_else(|e| panic!("{sql}: {e}"))
}

impl World {
    async fn new(db: PgPool) -> Self {
        let idp = TestIdp::start().await;
        let mut config = testing::config("postgres://test@localhost/test");
        config.auth = idp.auth_config();
        let pool = testing::app_role_pool(&db).await;
        let state = crate::with_ports(
            AppState::new(pool.clone(), config)
                .with_blobs(BlobStore::memory(&BlobsConfig::default(), pool))
                .with_registry(crate::registry().expect("valid registrations")),
        );
        let operations::Composed { routes, state, .. } = operations::compose(crate::api(), state);
        let app = routes
            .fallback(|| async { campbooks_kernel::Error::NotFound })
            .with_state(state.clone());
        let mut world = Self {
            db,
            app,
            state,
            idp,
            a: WorkspaceId::new(),
            b: WorkspaceId::new(),
            users: BTreeMap::new(),
            instances: BTreeMap::new(),
            inbox_ids: Vec::new(),
            run: String::new(),
        };
        world.seed().await;
        world
    }

    /// Seed A (every record) and B (its owner).
    async fn seed(&mut self) {
        self.seed_people().await;
        let (inbox, connection) = self.seed_mailbox().await;
        self.seed_records(inbox, connection).await;
    }

    /// Everyone: A's members, B's owner, a person in no workspace and an
    /// instance administrator.
    async fn seed_people(&mut self) {
        self.users.clear();
        let db = self.db.clone();
        let run = Uuid::now_v7().simple().to_string();
        self.a = workspace(&db, "Shop").await;
        self.b = workspace(&db, "Elsewhere").await;
        for who in MEMBERS.into_iter().filter(|w| *w != Who::Client) {
            let id = user(&db, &format!("{}-{run}", who.subject())).await;
            join(&db, self.a, id, who.role().unwrap()).await;
            self.users.insert(who, id);
        }
        let bo = user(&db, &format!("bo-{run}")).await;
        join(&db, self.b, bo, Role::Owner).await;
        user(&db, &format!("eve-{run}")).await;
        user(&db, &format!("ivy-{run}")).await;
        // An instance administrator, as `campbooks-server admin grant` makes one.
        let mut conn = db.acquire().await.unwrap();
        campbooks_admin::cli::grant(
            &mut conn,
            &format!("ivy-{run}"),
            &[campbooks_kernel::admin::Permission::Super],
            None,
            chrono::Utc::now(),
        )
        .await
        .unwrap();
        self.run.clone_from(&run);
    }

    /// The connector's mailbox: an inbox with the `en` fixture, fed by a
    /// connection.
    async fn seed_mailbox(&mut self) -> (campbooks_conversations::model::InboxId, Uuid) {
        let db = self.db.clone();
        let run = self.run.clone();
        let u = |who: Who| self.users[&who];
        let (connector, reader, sender) = (u(Who::Connector), u(Who::Reader), u(Who::Sender));
        let inbox = dev::create_inbox(
            &self.state,
            self.a,
            "ana@example.com",
            &[
                (connector, InboxRole::Manager),
                (reader, InboxRole::Reader),
                (sender, InboxRole::Sender),
            ],
        )
        .await
        .unwrap();
        let recorded: Value = serde_json::from_str(EN).unwrap();
        let batch = dev::record(&self.state, self.a, inbox, &recorded)
            .await
            .unwrap();
        dev::submit(&self.state, self.a, &batch).await.unwrap();
        Worker::new(self.state.clone()).run_until_idle().await;

        let connection = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO connectors.connections
                 (id, workspace_id, provider, account_subject, address, granted_by, capabilities)
             VALUES ($1, $2, 'google', $3, 'ana@example.com', $4, '{messages}')",
        )
        .bind(connection)
        .bind(self.a.as_uuid())
        .bind(format!("google-{run}"))
        .bind(connector.as_uuid())
        .execute(&db)
        .await
        .unwrap();
        sqlx::query("UPDATE conversations.inboxes SET connection_id = $2 WHERE id = $1")
            .bind(inbox)
            .bind(connection)
            .execute(&db)
            .await
            .unwrap();
        (inbox, connection)
    }

    /// A message (`en-1`), its conversation, an attachment, a decision and a
    /// label of `inbox`, as ingested.
    async fn inside(&self, inbox: campbooks_conversations::model::InboxId) -> [String; 5] {
        let db = &self.db;
        let message = scalar(
            db,
            "SELECT id::text FROM conversations.messages WHERE inbox_id = $1 AND provider_message_id = 'en-1'",
            *inbox.as_uuid(),
        )
        .await;
        let conversation = scalar(
            db,
            "SELECT conversation_id::text FROM conversations.messages WHERE inbox_id = $1 AND provider_message_id = 'en-1'",
            *inbox.as_uuid(),
        )
        .await;
        let attachment = scalar(
            db,
            "SELECT a.id::text FROM conversations.attachments a
               JOIN conversations.messages m ON m.id = a.message_id
              WHERE m.inbox_id = $1 LIMIT 1",
            *inbox.as_uuid(),
        )
        .await;
        let decision = scalar(
            db,
            "SELECT d.id::text FROM conversations.decisions d
               JOIN conversations.messages m ON m.id = d.message_id
              WHERE m.inbox_id = $1 LIMIT 1",
            *inbox.as_uuid(),
        )
        .await;
        let label = scalar(
            db,
            "SELECT id::text FROM conversations.labels WHERE inbox_id = $1 LIMIT 1",
            *inbox.as_uuid(),
        )
        .await;
        [message, conversation, attachment, decision, label]
    }

    /// One record of every other type, and what the inbox holds.
    async fn seed_records(
        &mut self,
        inbox: campbooks_conversations::model::InboxId,
        connection: Uuid,
    ) {
        let db = self.db.clone();
        let run = self.run.clone();
        let u = |who: Who| self.users[&who];
        let sender = u(Who::Sender);
        let [message, conversation, attachment, decision, label] = self.inside(inbox).await;
        let correction = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO conversations.corrections
                 (id, workspace_id, conversation_id, question, from_value, to_value, overridden_source)
             VALUES ($1, $2, $3::uuid, 'stream', 'primary', 'updates', 'default')",
        )
        .bind(correction)
        .bind(self.a.as_uuid())
        .bind(&conversation)
        .execute(&db)
        .await
        .unwrap();
        let draft = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO conversations.drafts (id, workspace_id, inbox_id, author, kind)
             VALUES ($1, $2, $3, $4, 'new')",
        )
        .bind(draft)
        .bind(self.a.as_uuid())
        .bind(inbox)
        .bind(sender.as_uuid())
        .execute(&db)
        .await
        .unwrap();
        let invitation = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO workspaces.invitations
                 (id, workspace_id, email, role, token_hash, expires_at, invited_by)
             VALUES ($1, $2, $3, 'member', $4, now() + interval '1 day', $5)",
        )
        .bind(invitation)
        .bind(self.a.as_uuid())
        .bind(format!("new-{run}@example.com"))
        .bind(Uuid::now_v7().as_bytes().to_vec())
        .bind(u(Who::Owner).as_uuid())
        .execute(&db)
        .await
        .unwrap();
        let event = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO workspaces.security_events (id, workspace_id, user_id, kind)
             VALUES ($1, $2, $3, 'member_joined')",
        )
        .bind(event)
        .bind(self.a.as_uuid())
        .bind(u(Who::Owner).as_uuid())
        .execute(&db)
        .await
        .unwrap();

        self.instances = BTreeMap::from([
            ("workspaces.workspace", self.a.to_string()),
            ("workspaces.member", u(Who::Member).to_string()),
            ("workspaces.invitation", invitation.to_string()),
            ("workspaces.security_event", event.to_string()),
            ("connectors.connection", connection.to_string()),
            ("conversations.inbox", inbox.to_string()),
            ("conversations.conversation", conversation.clone()),
            ("conversations.message", message.clone()),
            ("conversations.attachment", attachment.clone()),
            ("conversations.decision", decision.clone()),
            ("conversations.label", label.clone()),
            ("conversations.correction", correction.to_string()),
            ("conversations.draft", draft.to_string()),
        ]);
        let mut ids: Vec<String> = sqlx::query_scalar(
            "SELECT id::text FROM conversations.conversations WHERE inbox_id = $1
             UNION ALL SELECT id::text FROM conversations.messages WHERE inbox_id = $1",
        )
        .bind(inbox)
        .fetch_all(&db)
        .await
        .unwrap();
        ids.extend([inbox.to_string(), draft.to_string(), attachment, label]);
        self.inbox_ids = ids;
    }

    fn access(&self, who: Who) -> Access {
        Access {
            workspace: self.a,
            user: self.users.get(&who).copied(),
            role: who.role(),
        }
    }

    /// The truth about `resource_type`'s instance for `who`, as the seed
    /// made it.
    fn truth(&self, resource_type: &str, who: Who) -> Facts {
        let u = |w: Who| self.users[&w];
        let inbox = RecordRef::new(
            "conversations.inbox",
            &self.instances["conversations.inbox"],
        );
        let role = who.inbox_role().map(ToOwned::to_owned);
        match resource_type {
            "workspaces.member" => Facts::owned_by(OwnerFact::User(u(Who::Member))),
            "connectors.connection" => Facts::owned_by(OwnerFact::User(u(Who::Connector))),
            "conversations.inbox" => {
                Facts::owned_by(OwnerFact::User(u(Who::Connector))).within(inbox, role)
            }
            "conversations.draft" => {
                Facts::owned_by(OwnerFact::User(u(Who::Sender))).within(inbox, role)
            }
            name if name.starts_with("conversations.") => {
                Facts::owned_by(OwnerFact::Nobody).within(inbox, role)
            }
            _ => Facts::owned_by(OwnerFact::Nobody),
        }
    }

    fn token(&self, sub: &str) -> String {
        let mut claims = self.idp.claims(&all_scopes());
        claims["sub"] = json!(format!("{sub}-{}", self.run));
        self.idp.token(&claims)
    }

    fn client(&self, workspace: WorkspaceId) -> String {
        let mut claims = self.idp.claims(&all_scopes());
        let object = claims.as_object_mut().unwrap();
        object.remove("sub");
        object.insert("azp".into(), "acme-sync".into());
        object.insert("workspace_id".into(), workspace.to_string().into());
        self.idp.token(&claims)
    }

    async fn call(
        &self,
        token: Option<&str>,
        method: &str,
        uri: &str,
        body: Option<&Value>,
    ) -> (StatusCode, Vec<u8>) {
        let mut request = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let body = match body {
            Some(body) => {
                request = request.header(header::CONTENT_TYPE, "application/json");
                Body::from(body.to_string())
            }
            None => Body::empty(),
        };
        let response = self
            .app
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let streaming = response
            .headers()
            .get(header::CONTENT_TYPE)
            .is_some_and(|value| value.as_bytes().starts_with(b"text/event-stream"));
        if streaming {
            // The change feed never ends: its status is the answer.
            return (status, Vec::new());
        }
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, bytes.to_vec())
    }
}

// --- resources × principals × actions ------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn every_resource_answers_each_principal_as_its_catalogue_declares(db: PgPool) {
    let world = World::new(db).await;
    let catalogue = world.state.resources.clone();
    let workspace_types: Vec<_> = catalogue
        .iter()
        .filter(|declared| declared.tenancy == Tenancy::Workspace)
        .filter(|declared| !NOT_PER_INSTANCE.contains(&declared.name))
        .collect();
    let unseeded: Vec<&str> = workspace_types
        .iter()
        .map(|declared| declared.name)
        .filter(|name| !world.instances.contains_key(name))
        .collect();
    assert!(
        unseeded.is_empty(),
        "seed an instance of {unseeded:?} in the access matrix (World::seed)"
    );

    let mut checked = 0;
    for declared in &workspace_types {
        let resource = RecordRef::new(declared.name, &world.instances[declared.name]);
        for who in MEMBERS {
            let access = world.access(who);
            for action in Action::ALL {
                let mut tx = world.state.db.begin_for(world.a).await.unwrap();
                let actual = authz::authorize(&world.state, &mut tx, &access, action, &resource)
                    .await
                    .unwrap();
                tx.rollback().await.unwrap();
                let expected = authz::decide(
                    &catalogue,
                    &access,
                    action,
                    declared.name,
                    Some(&world.truth(declared.name, who)),
                );
                assert_eq!(
                    actual, expected,
                    "{who:?} {action} {}: the module's facts disagree with the seed",
                    declared.name
                );
                checked += 1;
            }
        }
        // A member of another workspace asks about it from theirs: it does
        // not exist for them.
        let elsewhere = Access {
            workspace: world.b,
            user: Some(UserId::new()),
            role: Some(Role::Owner),
        };
        let mut tx = world.state.db.begin_for(world.b).await.unwrap();
        for action in Action::ALL {
            let decision = authz::authorize(&world.state, &mut tx, &elsewhere, action, &resource)
                .await
                .unwrap();
            assert_eq!(decision, Decision::NotFound, "{} {action}", declared.name);
        }
        tx.rollback().await.unwrap();
    }
    assert!(checked > 0);

    // The owner's decisions, spelled out.
    let ask = |who: Who, action: Action, name: &'static str| {
        let world = &world;
        async move {
            let mut tx = world.state.db.begin_for(world.a).await.unwrap();
            let resource = RecordRef::new(name, &world.instances[name]);
            authz::authorize(&world.state, &mut tx, &world.access(who), action, &resource)
                .await
                .unwrap()
        }
    };
    for who in [Who::Owner, Who::Admin] {
        for name in [
            "conversations.inbox",
            "conversations.conversation",
            "conversations.message",
            "conversations.draft",
        ] {
            assert_eq!(
                ask(who, Action::Read, name).await,
                Decision::NotFound,
                "{who:?} {name}"
            );
        }
        assert_eq!(
            ask(who, Action::Disconnect, "connectors.connection").await,
            Decision::Forbidden,
            "only the member who connected a mailbox disconnects it"
        );
    }
    assert_eq!(
        ask(Who::Connector, Action::Disconnect, "connectors.connection").await,
        Decision::Allow
    );
    assert_eq!(
        ask(Who::Sender, Action::Send, "conversations.draft").await,
        Decision::Allow
    );
    assert_eq!(
        ask(Who::Connector, Action::Read, "conversations.draft").await,
        Decision::NotFound
    );
    assert_eq!(
        ask(Who::Reader, Action::Write, "conversations.conversation").await,
        Decision::Forbidden
    );
}

// --- operations × callers ------------------------------------------------------

/// A request body for every operation that takes one; an operation missing
/// here fails the matrix until it says how it is called.
fn body(world: &World, id: &str) -> Option<Value> {
    let inbox = &world.instances["conversations.inbox"];
    Some(match id {
        "workspaces.update" => json!({"name": "Renamed"}),
        "workspaces.invite_member" => json!({"email": "someone@example.com", "role": "member"}),
        "workspaces.change_role" => json!({"role": "admin"}),
        "workspaces.create" => json!({"name": "New", "kind": "business"}),
        "workspaces.accept_invitation" => json!({"token": "not-a-real-token"}),
        "connectors.start_connection" => {
            json!({"provider": "google", "capabilities": ["messages"]})
        }
        "conversations.add_label" => json!({"name": "Matrix"}),
        "conversations.move_to_stream" => json!({"stream": "updates"}),
        "conversations.create_draft" => json!({"kind": "new", "inbox_id": inbox}),
        "conversations.update_draft" => json!({"version": 1, "subject": "Matrix"}),
        "conversations.send_draft" => json!({"version": 1}),
        "identity.start_link" => json!({"purpose": "link", "provider": "google"}),
        "identity.register_passkey" => json!({"credential": {}}),
        "identity.rename_passkey" => json!({"name": "Laptop"}),
        "identity.register_client" => json!({"name": "Matrix agent", "scopes": []}),
        _ => return None,
    })
}

/// The instance an operation acts on: the one its path names for its
/// declared resource type (or, for creating a draft, the inbox in its
/// body); `None` for lists and creations.
fn instance_of(world: &World, id: &str, path: &str, resource: &str) -> Option<String> {
    let param = match resource {
        "workspaces.workspace" => "{workspace_id}",
        "workspaces.member" => "{user_id}",
        "connectors.connection" => "{connection_id}",
        "conversations.conversation" => "{conversation_id}",
        "conversations.message" => "{message_id}",
        "conversations.attachment" => "{attachment_id}",
        "conversations.draft" => "{draft_id}",
        "conversations.inbox" if id == "conversations.create_draft" => {
            return world.instances.get(resource).cloned();
        }
        _ => return None,
    };
    if id == "workspaces.leave" {
        return None; // the caller's own membership, below
    }
    path.contains(param)
        .then(|| world.instances.get(resource).cloned())
        .flatten()
}

/// The path with the seeded ids of workspace A.
fn fill(world: &World, path: &str) -> String {
    let message_conversation = &world.instances["conversations.conversation"];
    path.replace("{workspace_id}", &world.a.to_string())
        .replace("{conversation_id}", message_conversation)
        .replace("{message_id}", &world.instances["conversations.message"])
        .replace(
            "{attachment_id}",
            &world.instances["conversations.attachment"],
        )
        .replace("{draft_id}", &world.instances["conversations.draft"])
        .replace("{connection_id}", &world.instances["connectors.connection"])
        .replace("{label_id}", &world.instances["conversations.label"])
        .replace("{user_id}", &world.instances["workspaces.member"])
        .replace("{id}", &Uuid::now_v7().to_string())
}

/// What the catalogue says `who` gets from operation (`resource`, `action`)
/// on `instance` (or on the type, for lists and creations).
fn expected(
    world: &World,
    who: Who,
    resource: &str,
    action: Action,
    instance: Option<&str>,
    id: &str,
) -> Decision {
    let access = world.access(who);
    if id == "workspaces.leave" {
        // The caller's own membership.
        return if who == Who::Client {
            Decision::Forbidden
        } else {
            Decision::Allow
        };
    }
    match instance {
        Some(_) => authz::decide(
            &world.state.resources,
            &access,
            action,
            resource,
            Some(&world.truth(resource, who)),
        ),
        None => authz::authorize_type(&world.state.resources, &access, action, resource),
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn every_operation_answers_each_caller_as_declared(db: PgPool) {
    let mut world = World::new(db).await;
    let spec = serde_json::to_value(crate::openapi()).unwrap();
    // The 404 of a workspace that does not exist: what every caller who may
    // not see one gets, byte for byte.
    let eve = world.token("eve");
    let (status, canonical) = world
        .call(
            Some(&eve),
            "GET",
            &format!("/api/v1/w/{}/inboxes", WorkspaceId::new()),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let mut operations = Vec::new();
    for (path, item) in spec["paths"].as_object().unwrap() {
        for (method, operation) in item.as_object().unwrap() {
            let bearer = operation["security"]
                .as_array()
                .is_some_and(|reqs| reqs.iter().all(|r| r.get(crate::SECURITY_SCHEME).is_some()));
            if bearer {
                operations.push((path.clone(), method.to_uppercase(), operation.clone()));
            }
        }
    }
    assert!(operations.len() > 40, "{}", operations.len());
    let mut undeclared = Vec::new();
    for (_, _, operation) in &operations {
        let id = operation["operationId"].as_str().unwrap();
        if operation.get("requestBody").is_some() && body(&world, id).is_none() {
            undeclared.push(id.to_owned());
        }
    }
    assert!(
        undeclared.is_empty(),
        "say how the access matrix calls {undeclared:?} (fn body)"
    );

    let mut checks = 0;
    for (path, method, operation) in operations {
        // Every operation runs against fresh records: writes change them.
        world.seed().await;
        let id = operation["operationId"].as_str().unwrap().to_owned();
        let access = &operation["x-campbooks"]["access"];
        let resource = access["resource"].as_str().unwrap();
        let action: Action = serde_json::from_value(access["action"].clone()).unwrap();
        let mutates = operation["x-campbooks"]["mutates"].as_bool().unwrap();
        let in_workspace = path.contains("{workspace_id}");
        let what = format!("{method} {path} ({id})");
        let call = |world: &World| (fill(world, &path), body(world, &id));

        // Nobody signed in.
        let (uri, body_now) = call(&world);
        let (status, _) = world.call(None, &method, &uri, body_now.as_ref()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "anonymous: {what}");

        // Callers who may not see workspace A at all.
        let strangers = [
            ("signed in, in no workspace", world.token("eve")),
            ("a member of another workspace", world.token("bo")),
            ("an instance administrator", world.token("ivy")),
            ("a client token of another workspace", world.client(world.b)),
        ];
        for (who, token) in &strangers {
            let (status, bytes) = world
                .call(Some(token), &method, &uri, body_now.as_ref())
                .await;
            if in_workspace {
                assert_eq!(status, StatusCode::NOT_FOUND, "{who}: {what}");
                assert_eq!(bytes, canonical, "{who}: {what}: not the same 404");
            } else {
                assert!(!status.is_server_error(), "{who}: {what}: {status}");
            }
        }
        // Account resources (ways in, passkeys, held grants) answer only
        // their own account: the identity module's tests own that, until
        // they move onto per-instance facts (campbooks-cloud#117, PR B).
        let account = world
            .state
            .resources
            .get(resource)
            .is_some_and(|declared| declared.tenancy == Tenancy::Account);
        // Workspace clients answer only when the in-process issuer runs.
        if !in_workspace || account || resource == "identity.client" {
            continue;
        }

        // Each kind of member, and a client token of A. Callers who are
        // refused all ask the same records; each allowed caller gets fresh
        // ones when the operation writes.
        let instance = instance_of(&world, &id, &path, resource);
        let mut callers: Vec<(Who, Decision)> = MEMBERS
            .iter()
            .map(|who| {
                (
                    *who,
                    expected(&world, *who, resource, action, instance.as_deref(), &id),
                )
            })
            .collect();
        callers.sort_by_key(|(_, decision)| *decision == Decision::Allow);
        for (who, decision) in callers {
            if decision == Decision::Allow && mutates {
                world.seed().await;
            }
            let (uri, body_now) = call(&world);
            let token = match who {
                Who::Client => world.client(world.a),
                other => world.token(other.subject()),
            };
            let (status, bytes) = world
                .call(Some(&token), &method, &uri, body_now.as_ref())
                .await;
            let text = String::from_utf8_lossy(&bytes);
            checks += 1;
            match decision {
                Decision::NotFound => {
                    assert_eq!(status, StatusCode::NOT_FOUND, "{who:?}: {what}: {text}");
                    assert_eq!(bytes, canonical, "{who:?}: {what}: not the same 404");
                }
                Decision::Forbidden => {
                    assert_eq!(status, StatusCode::FORBIDDEN, "{who:?}: {what}: {text}");
                }
                Decision::Allow => {
                    assert!(
                        !matches!(
                            status,
                            StatusCode::UNAUTHORIZED
                                | StatusCode::FORBIDDEN
                                | StatusCode::NOT_FOUND
                        ),
                        "{who:?}: {what}: allowed, but {status} {text}"
                    );
                    assert!(
                        !status.is_server_error(),
                        "{who:?}: {what}: {status} {text}"
                    );
                }
            }
            // Nothing of an inbox a caller may not read leaks through a list.
            if status.is_success() && who.inbox_role().is_none() && who != Who::Client {
                for leaked in &world.inbox_ids {
                    assert!(
                        !text.contains(leaked.as_str()),
                        "{who:?}: {what}: leaked {leaked}"
                    );
                }
                let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
                assert_no_counts(&json, &format!("{who:?}: {what}"));
            }
        }
    }
    // Every kind of member and the client token, for every workspace
    // operation.
    assert!(checks > 250, "only {checks} member calls checked");
}

/// Counts and flags a caller could learn an invisible inbox's activity from
/// are zero (or false).
fn assert_no_counts(value: &Value, what: &str) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                if key.contains("count") || key == "total" {
                    assert!(
                        inner.as_u64().is_none_or(|n| n == 0),
                        "{what}: `{key}` = {inner}"
                    );
                }
                if key == "indexing" {
                    assert_ne!(inner, &Value::Bool(true), "{what}: indexing");
                }
                assert_no_counts(inner, what);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| assert_no_counts(item, what)),
        _ => {}
    }
}

#[test]
fn the_matrix_names_every_member_kind_once() {
    let unique: BTreeSet<&str> = MEMBERS.iter().map(|who| who.subject()).collect();
    assert_eq!(unique.len(), MEMBERS.len());
}
