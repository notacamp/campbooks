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
    /// A member sending from the inbox; the draft's and the rule's author.
    Sender,
    /// Another member sending from the inbox: author of neither.
    Colleague,
    /// A member who connected the mailbox (the inbox's owner and manager).
    Connector,
    /// A client token bound to A.
    Client,
    /// A client the admin registered: acts for the admin, nothing more.
    Agent,
}

const MEMBERS: [Who; 9] = [
    Who::Owner,
    Who::Admin,
    Who::Member,
    Who::Reader,
    Who::Sender,
    Who::Colleague,
    Who::Connector,
    Who::Client,
    Who::Agent,
];

impl Who {
    fn subject(self) -> &'static str {
        match self {
            Self::Owner => "wes",
            Self::Admin => "adi",
            Self::Member => "mo",
            Self::Reader => "rea",
            Self::Sender => "sen",
            Self::Colleague => "col",
            Self::Connector => "ana",
            Self::Client => "acme-sync",
            Self::Agent => "agent:adi",
        }
    }

    /// The person whose access this caller has.
    fn person(self) -> Option<Self> {
        match self {
            Self::Client => None,
            Self::Agent => Some(Self::Admin),
            other => Some(other),
        }
    }

    fn role(self) -> Option<Role> {
        match self {
            Self::Owner => Some(Role::Owner),
            Self::Admin | Self::Agent => Some(Role::Admin),
            Self::Client => None,
            _ => Some(Role::Member),
        }
    }

    fn inbox_role(self) -> Option<&'static str> {
        match self {
            Self::Reader => Some("reader"),
            Self::Sender | Self::Colleague => Some("sender"),
            Self::Connector => Some("manager"),
            _ => None,
        }
    }
}

/// What became of the connector's mail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mail {
    /// Connected and syncing.
    Live,
    /// Disconnected with `?mail=keep`: read-only history.
    Kept,
    /// Disconnected with `?mail=delete`: gone.
    Removed,
}

/// Two workspaces with every kind of record, and the app.
struct World {
    /// What became of the inbox's mail.
    mail: Mail,
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

/// Registered clients of the test world: `agent:<subject>` acts for the
/// person with that subject (as a member's workspace client does); any
/// other client is unregistered.
#[derive(Debug)]
struct Agents;

impl campbooks_kernel::tenancy::ClientDirectory for Agents {
    fn acting_for<'a>(
        &'a self,
        _db: &'a PgPool,
        client_id: &'a str,
        _workspace: WorkspaceId,
    ) -> campbooks_kernel::tenancy::ClientsFuture<'a> {
        use campbooks_kernel::tenancy::ClientActor;
        let actor = client_id
            .strip_prefix("agent:")
            .map_or(ClientActor::Unregistered, |subject| {
                ClientActor::ActsFor(subject.to_owned())
            });
        Box::pin(async move { Ok(actor) })
    }
}

impl World {
    async fn new(db: PgPool) -> Self {
        let idp = TestIdp::start().await;
        let mut config = testing::config("postgres://test@localhost/test");
        config.auth = idp.auth_config();
        let pool = testing::app_role_pool(&db).await;
        let state = crate::with_ports(
            AppState::new(pool.clone(), config)
                .with_binding(testing::binding())
                .with_blobs(BlobStore::memory(&BlobsConfig::default(), pool))
                .with_registry(crate::registry().expect("valid registrations")),
        )
        .with_client_directory(std::sync::Arc::new(Agents));
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
            mail: Mail::Live,
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
        self.mail = Mail::Live;
    }

    /// Seed, then disconnect the mailbox as its connector does, keeping or
    /// deleting its mail.
    async fn seed_disconnected(&mut self, mail: Mail) {
        self.seed().await;
        let choice = match mail {
            Mail::Live => return,
            Mail::Kept => "keep",
            Mail::Removed => "delete",
        };
        let uri = format!(
            "/api/v1/w/{}/connections/{}?mail={choice}",
            self.a, self.instances["connectors.connection"]
        );
        let token = self.token(Who::Connector.subject());
        let (status, bytes, _) = self.call(Some(&token), "DELETE", &uri, None).await;
        assert_eq!(
            status,
            StatusCode::NO_CONTENT,
            "{}",
            String::from_utf8_lossy(&bytes)
        );
        Worker::new(self.state.clone()).run_until_idle().await;
        self.mail = mail;
    }

    /// Everyone: A's members, B's owner, a person in no workspace and an
    /// instance administrator.
    async fn seed_people(&mut self) {
        self.users.clear();
        let db = self.db.clone();
        let run = Uuid::now_v7().simple().to_string();
        self.a = workspace(&db, "Shop").await;
        self.b = workspace(&db, "Elsewhere").await;
        for who in MEMBERS.into_iter().filter(|w| w.person() == Some(*w)) {
            let id = user(&db, &format!("{}-{run}", who.subject())).await;
            join(&db, self.a, id, who.role().unwrap()).await;
            self.users.insert(who, id);
        }
        let bo = user(&db, &format!("bo-{run}")).await;
        join(&db, self.b, bo, Role::Owner).await;
        user(&db, &format!("eve-{run}")).await;
        // A member who left: their row stays, their membership is gone.
        let gone = user(&db, &format!("gone-{run}")).await;
        join(&db, self.a, gone, Role::Member).await;
        sqlx::query("DELETE FROM workspaces.members WHERE user_id = $1")
            .bind(gone.as_uuid())
            .execute(&db)
            .await
            .unwrap();
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
        let colleague = u(Who::Colleague);
        let inbox = dev::create_inbox(
            &self.state,
            self.a,
            "ana@example.com",
            &[
                (connector, InboxRole::Manager),
                (reader, InboxRole::Reader),
                (sender, InboxRole::Sender),
                (colleague, InboxRole::Sender),
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

    /// What needs the inbox's readers: an attention item about the
    /// conversation.
    async fn seed_item(
        &self,
        inbox: campbooks_conversations::model::InboxId,
        conversation: &str,
        message: &str,
    ) -> Uuid {
        let item = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO attention.items
                 (id, workspace_id, record_type, record_id, container_type, container_id,
                  cause_type, cause_id, reason, trust, since, state, producer_revision)
             VALUES ($1, $2, 'conversations.conversation', $3, 'conversations.inbox', $4,
                     'conversations.message', $5, 'reply', 'sure', now(), 'open', 1)",
        )
        .bind(item)
        .bind(self.a.as_uuid())
        .bind(conversation)
        .bind(inbox.to_string())
        .bind(message)
        .execute(&self.db)
        .await
        .unwrap();
        item
    }

    /// A rule the sender taught the inbox (campbooks-cloud#125).
    async fn seed_rule(
        &self,
        inbox: campbooks_conversations::model::InboxId,
        sender: UserId,
    ) -> Uuid {
        let rule = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO conversations.rules
                 (id, workspace_id, inbox_id, teaching_id, conditions, actions, specificity,
                  origin, state, taught_by, confirmed_at)
             VALUES ($1, $2, $3, $4,
                     '[{\"field\": \"sender_address\", \"value\": \"news@example.com\"}]',
                     '[{\"action\": \"archive\"}]', 2, 'action', 'active', $5, now())",
        )
        .bind(rule)
        .bind(self.a.as_uuid())
        .bind(inbox)
        .bind(Uuid::now_v7())
        .bind(sender.as_uuid())
        .execute(&self.db)
        .await
        .unwrap();
        rule
    }

    /// What `member` told Scout about a person (campbooks-cloud#272).
    async fn seed_note(&self, member: UserId) -> Uuid {
        let note = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO conversations.person_notes (id, workspace_id, user_id, address, note)
             VALUES ($1, $2, $3, 'marta@clientes.example', 'My landlord. Formal, in Portuguese.')",
        )
        .bind(note)
        .bind(self.a.as_uuid())
        .bind(member.as_uuid())
        .execute(&self.db)
        .await
        .unwrap();
        note
    }

    /// `member`'s chat with Scout, about nothing (campbooks-cloud#305), with
    /// a question.
    async fn seed_chat(&self, member: UserId) -> Uuid {
        let chat = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO assistant.chats (id, workspace_id, user_id, questions)
             VALUES ($1, $2, $3, 1)",
        )
        .bind(chat)
        .bind(self.a.as_uuid())
        .bind(member.as_uuid())
        .execute(&self.db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO assistant.chat_messages (id, workspace_id, chat_id, author, text)
             VALUES ($1, $2, $3, 'member', 'Why is this here?')",
        )
        .bind(Uuid::now_v7())
        .bind(self.a.as_uuid())
        .bind(chat)
        .execute(&self.db)
        .await
        .unwrap();
        chat
    }

    /// Something Scout noticed in the inbox (campbooks-cloud#169).
    async fn seed_notice(&self, inbox: campbooks_conversations::model::InboxId) -> Uuid {
        let notice = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO conversations.rule_notices
                 (id, workspace_id, inbox_id, kind, sender_address, conditions, actions, evidence)
             VALUES ($1, $2, $3, 'archive_on_arrival', 'digest@example.com',
                     '[{\"field\": \"sender_address\", \"value\": \"digest@example.com\"}]',
                     '[{\"action\": \"archive\"}]',
                     '{\"received\": 12, \"opened\": 0, \"archived_unread\": 12, \"last_week\": 2, \"window_days\": 60}')",
        )
        .bind(notice)
        .bind(self.a.as_uuid())
        .bind(inbox)
        .execute(&self.db)
        .await
        .unwrap();
        notice
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
        let item = self.seed_item(inbox, &conversation, &message).await;
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
        let rule = self.seed_rule(inbox, sender).await;
        let notice = self.seed_notice(inbox).await;
        let note = self.seed_note(u(Who::Member)).await;
        let chat = self.seed_chat(u(Who::Member)).await;
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
            ("conversations.rule", rule.to_string()),
            // Not a resource of its own: answered as its inbox.
            ("conversations.rule_notice", notice.to_string()),
            ("attention.item", item.to_string()),
            // A profile is the member's own, addressed by their user id.
            ("assistant.profile", u(Who::Member).to_string()),
            // What a member told Scout about a person: theirs alone, in no
            // inbox (campbooks-cloud#272).
            ("conversations.person_note", note.to_string()),
            // A member's chat with Scout: theirs alone (campbooks-cloud#305).
            ("assistant.chat", chat.to_string()),
        ]);
        let mut ids: Vec<String> = sqlx::query_scalar(
            "SELECT id::text FROM conversations.conversations WHERE inbox_id = $1
             UNION ALL SELECT id::text FROM conversations.messages WHERE inbox_id = $1",
        )
        .bind(inbox)
        .fetch_all(&db)
        .await
        .unwrap();
        ids.extend([
            inbox.to_string(),
            draft.to_string(),
            rule.to_string(),
            item.to_string(),
            attachment,
            label,
        ]);
        self.inbox_ids = ids;
    }

    fn access(&self, who: Who) -> Access {
        Access {
            workspace: self.a,
            user: who.person().and_then(|p| self.users.get(&p).copied()),
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
        let role = who
            .person()
            .and_then(Who::inbox_role)
            .map(ToOwned::to_owned);
        match resource_type {
            // A profile is addressed by its member, like a membership; a
            // person note is the member's, in no inbox.
            "workspaces.member"
            | "assistant.profile"
            | "assistant.chat"
            | "conversations.person_note" => Facts::owned_by(OwnerFact::User(u(Who::Member))),
            "connectors.connection" => Facts::owned_by(OwnerFact::User(u(Who::Connector))),
            "conversations.inbox" => {
                Facts::owned_by(OwnerFact::User(u(Who::Connector))).within(inbox, role)
            }
            "conversations.draft" | "conversations.rule" => {
                Facts::owned_by(OwnerFact::User(u(Who::Sender))).within(inbox, role)
            }
            // Everything else inside the inbox, this module's or another's.
            "attention.item" => Facts::owned_by(OwnerFact::Nobody)
                .within(inbox, None)
                .visible(role.is_some()),
            name if name.starts_with("conversations.") => {
                Facts::owned_by(OwnerFact::Nobody).within(inbox, role)
            }
            _ => Facts::owned_by(OwnerFact::Nobody),
        }
        .frozen(self.mail == Mail::Kept && in_the_inbox(resource_type))
    }

    fn token(&self, sub: &str) -> String {
        let mut claims = self.idp.claims(&all_scopes());
        claims["sub"] = json!(format!("{sub}-{}", self.run));
        self.idp.token(&claims)
    }

    /// A client token bound to A, registered by the person `sub`.
    fn agent(&self, sub: &str) -> String {
        let mut claims = self.idp.claims(&all_scopes());
        let object = claims.as_object_mut().unwrap();
        object.remove("sub");
        let id = format!("agent:{sub}-{}", self.run);
        object.insert("azp".into(), id.into());
        object.insert("workspace_id".into(), self.a.to_string().into());
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
    ) -> (StatusCode, Vec<u8>, String) {
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
        // The headers a client could tell two answers apart by.
        let heads = [
            header::CONTENT_TYPE,
            header::CONTENT_LANGUAGE,
            header::WWW_AUTHENTICATE,
        ]
        .iter()
        .map(|name| {
            let value = response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default();
            format!("{name}: {value}")
        })
        .collect::<Vec<_>>()
        .join("\n");
        let streaming = response
            .headers()
            .get(header::CONTENT_TYPE)
            .is_some_and(|value| value.as_bytes().starts_with(b"text/event-stream"));
        if streaming {
            // The change feed never ends: its status is the answer.
            return (status, Vec::new(), heads);
        }
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, bytes.to_vec(), heads)
    }
}

// --- resources × principals × actions ------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn every_resource_answers_each_principal_as_its_catalogue_declares(db: PgPool) {
    let world = World::new(db).await;
    let catalogue = world.state.resources.clone();
    assert_eq!(catalogue.missing_containers(), Vec::<&str>::new());
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
        "connectors.discover_imap" => json!({"address": "matrix@example.com"}),
        "connectors.connect_imap" => {
            json!({"address": "matrix@example.com", "password": "not-a-real-password"})
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
        "system.report_client_telemetry" => json!({"session_id": "matrix"}),
        "system.report_screen_view" => json!({"route": "/inbox"}),
        "attention.snooze" => json!({"until": chrono::Utc::now() + chrono::TimeDelta::days(1)}),
        "conversations.resolve_need" => {
            json!({"outcome": "done", "message_id": world.instances["conversations.message"]})
        }
        "conversations.confirm_need" => {
            json!({"message_id": world.instances["conversations.message"]})
        }
        "conversations.preview_rule" | "conversations.create_rule" => json!({
            "conditions": [{"field": "sender_address", "value": "matrix@example.com"}],
            "actions": [{"action": "archive"}],
            "apply_to_existing": false,
        })
        .as_object()
        .map(|rule| {
            let mut rule = rule.clone();
            if id == "conversations.preview_rule" {
                rule.remove("apply_to_existing");
            }
            Value::Object(rule)
        })
        .unwrap_or_default(),
        "conversations.update_rule" => json!({"actions": [{"action": "mark_read"}]}),
        "conversations.set_person_note" => {
            json!({"address": "matrix@example.com", "note": "A client. Formal."})
        }
        "conversations.confirm_rule"
        | "conversations.restore_rule"
        | "conversations.suggest_replies"
        | "conversations.write_nudge"
        | "assistant.start_chat" => json!({}),
        "conversations.dismiss_rule_suggestion" => json!({"key": "matrix"}),
        "conversations.answer_rule_notice" => json!({"answer": "not_now"}),
        "assistant.update_profile" => json!({"proactivity": "ask_first", "asked": true}),
        "assistant.compile_instruction" => json!({"text": "newsletters never need me"}),
        "assistant.ask_chat" | "assistant.stream_chat" => json!({"text": "Why is this here?"}),
        "conversations.write_reply" | "conversations.stream_reply" => {
            json!({"intent": "Say yes"})
        }
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
        "conversations.rule" => "{rule_id}",
        "conversations.person_note" => "{note_id}",
        "assistant.chat" => "{chat_id}",
        "attention.item" => "{item_id}",
        "conversations.inbox" if acts_in_the_inbox(id) => {
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

/// Operations declared on the inbox that act on one instance of it (the
/// inbox in their path or body, or the inbox of the conversation they name).
fn acts_in_the_inbox(id: &str) -> bool {
    matches!(
        id,
        "conversations.create_draft"
            | "conversations.confirm_rule"
            | "conversations.decline_rule"
            | "conversations.preview_rule"
            | "conversations.create_rule"
            | "conversations.suggest_rules"
            | "conversations.dismiss_rule_suggestion"
            | "conversations.suggest_replies"
            | "conversations.write_reply"
            | "conversations.write_nudge"
            | "conversations.stream_reply"
            | "conversations.answer_rule_notice"
            | "conversations.clear_rule_notice_answer"
    )
}

/// Whether a resource type lives inside the inbox (its module's records
/// but a person note, which is a member's own, campbooks-cloud#272).
fn in_the_inbox(resource_type: &str) -> bool {
    resource_type.starts_with("conversations.") && resource_type != "conversations.person_note"
}

/// Operations on one of Scout's suggestions (campbooks-cloud#169).
fn answers_a_notice(id: &str) -> bool {
    matches!(
        id,
        "conversations.answer_rule_notice" | "conversations.clear_rule_notice_answer"
    )
}

/// The query string an operation needs to be understood at all.
fn query_of(id: &str) -> &'static str {
    match id {
        "conversations.suggest_rules" => "?action=archive",
        "conversations.list_correspondents" => "?q=example",
        _ => "",
    }
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
        .replace("{item_id}", &world.instances["attention.item"])
        .replace("{inbox_id}", &world.instances["conversations.inbox"])
        .replace("{rule_id}", &world.instances["conversations.rule"])
        .replace("{notice_id}", &world.instances["conversations.rule_notice"])
        .replace("{note_id}", &world.instances["conversations.person_note"])
        .replace("{chat_id}", &world.instances["assistant.chat"])
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
    let (status, canonical, canonical_heads) = world
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
        let call = |world: &World| {
            (
                format!("{}{}", fill(world, &path), query_of(&id)),
                body(world, &id),
            )
        };

        // Nobody signed in.
        let (uri, body_now) = call(&world);
        let (status, _, _) = world.call(None, &method, &uri, body_now.as_ref()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "anonymous: {what}");

        // Callers who may not see workspace A at all.
        let strangers = [
            ("signed in, in no workspace", world.token("eve")),
            ("a member who left", world.token("gone")),
            ("a member of another workspace", world.token("bo")),
            ("an instance administrator", world.token("ivy")),
            ("a client token of another workspace", world.client(world.b)),
        ];
        for (who, token) in &strangers {
            let (status, bytes, heads) = world
                .call(Some(token), &method, &uri, body_now.as_ref())
                .await;
            if in_workspace {
                assert_eq!(status, StatusCode::NOT_FOUND, "{who}: {what}");
                assert_eq!(bytes, canonical, "{who}: {what}: not the same 404");
                assert_eq!(
                    heads, canonical_heads,
                    "{who}: {what}: not the same 404 headers"
                );
            } else {
                assert!(!status.is_server_error(), "{who}: {what}: {status}");
            }
        }
        if in_workspace {
            // A client a member who left registered: refused outright.
            let (status, _, _) = world
                .call(Some(&world.agent("gone")), &method, &uri, body_now.as_ref())
                .await;
            assert_eq!(
                status,
                StatusCode::UNAUTHORIZED,
                "a departed member's client: {what}"
            );
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
                Who::Agent => world.agent(Who::Admin.subject()),
                other => world.token(other.subject()),
            };
            let (status, bytes, heads) = world
                .call(Some(&token), &method, &uri, body_now.as_ref())
                .await;
            let text = String::from_utf8_lossy(&bytes);
            checks += 1;
            match decision {
                Decision::NotFound => {
                    assert_eq!(status, StatusCode::NOT_FOUND, "{who:?}: {what}: {text}");
                    assert_eq!(bytes, canonical, "{who:?}: {what}: not the same 404");
                    assert_eq!(
                        heads, canonical_heads,
                        "{who:?}: {what}: not the same 404 headers"
                    );
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
            if status.is_success() && who.inbox_role().is_none() {
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

/// The inbox's records once its connection is gone: kept mail answers reads
/// and deletions only, deleted mail answers nobody, and neither leaks
/// through a list.
#[sqlx::test(migrations = "../../migrations")]
async fn a_disconnected_mailbox_answers_as_declared(db: PgPool) {
    let mut world = World::new(db).await;
    let spec = serde_json::to_value(crate::openapi()).unwrap();
    let eve = world.token("eve");
    let (_, canonical, canonical_heads) = world
        .call(
            Some(&eve),
            "GET",
            &format!("/api/v1/w/{}/inboxes", WorkspaceId::new()),
            None,
        )
        .await;
    let mut checks = 0;
    for mail in [Mail::Kept, Mail::Removed] {
        for (path, item) in spec["paths"].as_object().unwrap() {
            for (method, operation) in item.as_object().unwrap() {
                let method = method.to_uppercase();
                let access = &operation["x-campbooks"]["access"];
                let Some(resource) = access["resource"].as_str() else {
                    continue;
                };
                let inside = in_the_inbox(resource)
                    || ["attention.", "search."]
                        .iter()
                        .any(|module| resource.starts_with(module));
                if !path.contains("{workspace_id}") || !inside {
                    continue;
                }
                let id = operation["operationId"].as_str().unwrap().to_owned();
                let action: Action = serde_json::from_value(access["action"].clone()).unwrap();
                let what = format!("{mail:?}: {method} {path} ({id})");
                world.seed_disconnected(mail).await;
                let instance = instance_of(&world, &id, path, resource);
                // Callers who are refused all ask the same records; each
                // allowed caller (last) gets fresh ones when they may change.
                let mut callers: Vec<(Who, Decision)> = MEMBERS
                    .iter()
                    .map(|who| {
                        let decision = match (mail, &instance) {
                            // Nothing of a deleted inbox is anybody's any more.
                            (Mail::Removed, Some(_))
                                if resource != "conversations.inbox" || acts_in_the_inbox(&id) =>
                            {
                                Decision::NotFound
                            }
                            // A kept inbox forgets what Scout noticed in it
                            // (`forget_connection`): its suggestions are
                            // nobody's any more either.
                            (Mail::Kept, Some(_)) if answers_a_notice(&id) => Decision::NotFound,
                            _ => expected(&world, *who, resource, action, instance.as_deref(), &id),
                        };
                        (*who, decision)
                    })
                    .collect();
                callers.sort_by_key(|(_, decision)| *decision == Decision::Allow);
                for (who, decision) in callers {
                    if decision == Decision::Allow && mail == Mail::Kept {
                        world.seed_disconnected(mail).await;
                    }
                    let uri = format!("{}{}", fill(&world, path), query_of(&id));
                    let body_now = body(&world, &id);
                    let token = match who {
                        Who::Client => world.client(world.a),
                        Who::Agent => world.agent(Who::Admin.subject()),
                        other => world.token(other.subject()),
                    };
                    let (status, bytes, heads) = world
                        .call(Some(&token), &method, &uri, body_now.as_ref())
                        .await;
                    let text = String::from_utf8_lossy(&bytes);
                    checks += 1;
                    match decision {
                        Decision::NotFound => {
                            assert_eq!(status, StatusCode::NOT_FOUND, "{who:?}: {what}: {text}");
                            assert_eq!(bytes, canonical, "{who:?}: {what}: not the same 404");
                            assert_eq!(heads, canonical_heads, "{who:?}: {what}: 404 headers");
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
                    let blind =
                        mail == Mail::Removed || who.person().and_then(Who::inbox_role).is_none();
                    if status.is_success() && blind {
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
        }
    }
    assert!(checks > 100, "only {checks} calls checked");
}

/// A scheduled send against the real membership (`workspaces`'
/// `role_held`): released while its author is a member; back to a draft
/// once they are not, even before their departure is handled.
#[sqlx::test(migrations = "../../migrations")]
async fn a_scheduled_send_follows_its_authors_real_membership(db: PgPool) {
    let mut world = World::new(db).await;
    for still_a_member in [true, false] {
        world.seed().await;
        let draft = world.instances["conversations.draft"].clone();
        let uri = format!("/api/v1/w/{}/drafts/{draft}", world.a);
        let sender = world.token(Who::Sender.subject());
        let (_, read, _) = world.call(Some(&sender), "GET", &uri, None).await;
        let read: Value = serde_json::from_slice(&read).unwrap();
        let body = json!({"version": 1, "subject": "Matrix", "to": [{"address": "leo@example.com"}],
            "content_token": read["content_token"]});
        let (status, bytes, _) = world.call(Some(&sender), "PATCH", &uri, Some(&body)).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(&bytes)
        );
        let saved: Value = serde_json::from_slice(&bytes).unwrap();
        let send = json!({"version": 2, "content_token": saved["content_token"]});
        let (status, bytes, _) = world
            .call(Some(&sender), "POST", &format!("{uri}/send"), Some(&send))
            .await;
        assert_eq!(
            status,
            StatusCode::ACCEPTED,
            "{}",
            String::from_utf8_lossy(&bytes)
        );
        if !still_a_member {
            // Removed; `member_left` not handled yet (inbox membership kept).
            sqlx::query("DELETE FROM workspaces.members WHERE user_id = $1")
                .bind(world.users[&Who::Sender].as_uuid())
                .execute(&world.db)
                .await
                .unwrap();
        }
        sqlx::query(
            "UPDATE kernel.jobs SET run_at = now()
              WHERE kind = 'conversations.release_send' AND state = 'pending'",
        )
        .execute(&world.db)
        .await
        .unwrap();
        Worker::new(world.state.clone()).run_until_idle().await;
        let state: String =
            sqlx::query_scalar("SELECT state FROM conversations.drafts WHERE id = $1::uuid")
                .bind(&draft)
                .fetch_one(&world.db)
                .await
                .unwrap();
        let requested: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM kernel.events
              WHERE name = 'conversations.send_requested' AND workspace_id = $1",
        )
        .bind(world.a.to_string())
        .fetch_one(&world.db)
        .await
        .unwrap();
        if still_a_member {
            // Released (the test mailbox can't deliver it, so it may end
            // `failed` once the connector answers).
            assert_ne!(state, "draft");
            assert_eq!(requested, 1);
            // The connector asked conversations through the real
            // outbound-requests port, which confirmed it (campbooks-cloud#226).
            assert_eq!(sends_recorded(&world).await, 1);
        } else {
            assert_eq!(state, "draft");
            assert_eq!(requested, 0);
        }
    }
}

/// campbooks-cloud#242, through the real ports: the sender's scheduled send
/// once a messages stream feeds its inbox, with that stream pointed at
/// another mailbox by SQL between the send and its release when `repoint`.
/// The draft's state then, the `send_requested` events and the sends the
/// connector recorded.
async fn send_through_a_stream(db: PgPool, repoint: bool) -> (String, i64, i64) {
    let world = World::new(db).await;
    let stream = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO connectors.streams
             (id, workspace_id, connection_id, capability, phase, interval_seconds, inbox_id)
         VALUES ($1, $2, $3::uuid, 'messages', 'incremental', 60, $4::uuid)",
    )
    .bind(stream)
    .bind(world.a.as_uuid())
    .bind(&world.instances["connectors.connection"])
    .bind(&world.instances["conversations.inbox"])
    .execute(&world.db)
    .await
    .unwrap();
    let draft = world.instances["conversations.draft"].clone();
    let uri = format!("/api/v1/w/{}/drafts/{draft}", world.a);
    let sender = world.token(Who::Sender.subject());
    let (_, read, _) = world.call(Some(&sender), "GET", &uri, None).await;
    let read: Value = serde_json::from_slice(&read).unwrap();
    let body = json!({"version": 1, "subject": "Matrix", "to": [{"address": "leo@example.com"}],
        "content_token": read["content_token"]});
    let (status, bytes, _) = world.call(Some(&sender), "PATCH", &uri, Some(&body)).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let saved: Value = serde_json::from_slice(&bytes).unwrap();
    let send = json!({"version": 2, "content_token": saved["content_token"]});
    let (status, bytes, _) = world
        .call(Some(&sender), "POST", &format!("{uri}/send"), Some(&send))
        .await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    if repoint {
        sqlx::query("UPDATE connectors.streams SET mailbox_key = 'another-mailbox' WHERE id = $1")
            .bind(stream)
            .execute(&world.db)
            .await
            .unwrap();
    }
    sqlx::query(
        "UPDATE kernel.jobs SET run_at = now()
          WHERE kind = 'conversations.release_send' AND state = 'pending'",
    )
    .execute(&world.db)
    .await
    .unwrap();
    Worker::new(world.state.clone()).run_until_idle().await;
    let state: String =
        sqlx::query_scalar("SELECT state FROM conversations.drafts WHERE id = $1::uuid")
            .bind(&draft)
            .fetch_one(&world.db)
            .await
            .unwrap();
    let requested: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.events
          WHERE name = 'conversations.send_requested' AND workspace_id = $1",
    )
    .bind(world.a.to_string())
    .fetch_one(&world.db)
    .await
    .unwrap();
    (state, requested, sends_recorded(&world).await)
}

/// campbooks-cloud#242: a send goes out where the member sent it. The
/// author's request names the inbox's stream, connection and mailbox, the
/// release verifies it against them and binds the stored send to the same,
/// and the connector records it through that stream.
#[sqlx::test(migrations = "../../migrations")]
async fn a_scheduled_send_is_released_to_where_its_member_sent_it(db: PgPool) {
    let (state, requested, recorded) = send_through_a_stream(db, false).await;
    assert_ne!(state, "scheduled");
    assert_eq!((requested, recorded), (1, 1));
}

/// The stream pointed at another mailbox by SQL after the member asked: the
/// release refuses it (a WARN), so nothing is requested, recorded or sent,
/// and the draft is left as it is.
#[sqlx::test(migrations = "../../migrations")]
async fn a_scheduled_send_whose_stream_was_repointed_is_never_released(db: PgPool) {
    let (state, requested, recorded) = send_through_a_stream(db, true).await;
    assert_eq!(state, "scheduled");
    assert_eq!((requested, recorded), (0, 0));
}

/// The sends the connector recorded in workspace A.
async fn sends_recorded(world: &World) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM connectors.sends WHERE workspace_id = $1")
        .bind(world.a.as_uuid())
        .fetch_one(&world.db)
        .await
        .unwrap()
}

/// campbooks-cloud#226: `conversations.send_requested` appended in workspace
/// A by a transaction of the runtime role, naming a real draft that was
/// never released, is checked by the connector against the real
/// outbound-requests port: nothing is recorded, so nothing can be sent.
#[sqlx::test(migrations = "../../migrations")]
async fn a_forged_send_request_is_refused_by_the_real_port(db: PgPool) {
    use campbooks_conversations::{
        events::SendRequested,
        model::{DraftId, InboxId, SendId},
    };
    let world = World::new(db).await;
    let ws = world.a.to_string();
    let message = world
        .state
        .blobs
        .put(&ws, b"{}".to_vec(), "application/json")
        .await
        .unwrap();
    let mut tx = world.state.db.begin_for(world.a).await.unwrap();
    world
        .state
        .events
        .publish(
            &mut tx,
            &SendRequested {
                workspace: ws.clone(),
                inbox_id: world.instances["conversations.inbox"]
                    .parse::<InboxId>()
                    .unwrap(),
                draft: world.instances["conversations.draft"]
                    .parse::<DraftId>()
                    .unwrap(),
                send_id: SendId::new(),
                message,
                author: Some(world.users[&Who::Sender]),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    Worker::new(world.state.clone()).run_until_idle().await;
    assert_eq!(sends_recorded(&world).await, 0);
}

/// campbooks-cloud#227: with SQL as the runtime role in workspace A, a
/// scheduled draft of the attacker's own (a member who may send as its
/// author, a matching generation) and a planted `conversations.release_send`
/// job: the real release verifies the author's binding first, so nothing is
/// released and the connector records no send.
#[sqlx::test(migrations = "../../migrations")]
async fn a_planted_scheduled_send_is_never_released(db: PgPool) {
    let world = World::new(db).await;
    let draft = world.instances["conversations.draft"].clone();
    let mut tx = world.state.db.begin_for(world.a).await.unwrap();
    sqlx::query(
        "UPDATE conversations.drafts
            SET to_recipients = '[{\"address\": \"someone@example.net\"}]',
                subject = 'Invoice', body = 'Pay to this account', state = 'scheduled',
                send_generation = 1, send_at = now()
          WHERE id = $1::uuid",
    )
    .bind(&draft)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO kernel.jobs (queue, kind, payload, workspace_id, max_attempts)
         VALUES ('default', 'conversations.release_send', $1, $2, 3)",
    )
    .bind(json!({"draft": draft, "generation": 1}))
    .bind(world.a.to_string())
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    Worker::new(world.state.clone()).run_until_idle().await;
    let requested: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.events
          WHERE name = 'conversations.send_requested' AND workspace_id = $1",
    )
    .bind(world.a.to_string())
    .fetch_one(&world.db)
    .await
    .unwrap();
    assert_eq!(requested, 0);
    let (state, document): (String, bool) = sqlx::query_as(
        "SELECT state, send_document IS NOT NULL FROM conversations.drafts WHERE id = $1::uuid",
    )
    .bind(&draft)
    .fetch_one(&world.db)
    .await
    .unwrap();
    assert_eq!((state.as_str(), document), ("scheduled", false));
    assert_eq!(sends_recorded(&world).await, 0);
}

/// campbooks-cloud#227: a `sending` draft planted by SQL with a document
/// and a made-up binding, and the `send_requested` naming it: the real port
/// verifies the stored send's binding, so the connector records nothing.
#[sqlx::test(migrations = "../../migrations")]
async fn a_planted_stored_send_is_refused_by_the_real_port(db: PgPool) {
    use campbooks_conversations::{
        events::SendRequested,
        model::{DraftId, InboxId, SendId},
    };
    let world = World::new(db).await;
    let ws = world.a.to_string();
    let draft = world.instances["conversations.draft"].clone();
    let message = world
        .state
        .blobs
        .put(&ws, b"{}".to_vec(), "application/json")
        .await
        .unwrap();
    let send_id = SendId::new();
    let mut tx = world.state.db.begin_for(world.a).await.unwrap();
    sqlx::query(
        "UPDATE conversations.drafts
            SET state = 'sending', send_id = $2, send_document = $3,
                send_mac = '\\x01'::bytea || sha256('forged')
          WHERE id = $1::uuid",
    )
    .bind(&draft)
    .bind(send_id)
    .bind(serde_json::to_value(&message).unwrap())
    .execute(&mut *tx)
    .await
    .unwrap();
    world
        .state
        .events
        .publish(
            &mut tx,
            &SendRequested {
                workspace: ws.clone(),
                inbox_id: world.instances["conversations.inbox"]
                    .parse::<InboxId>()
                    .unwrap(),
                draft: draft.parse::<DraftId>().unwrap(),
                send_id,
                message,
                author: Some(world.users[&Who::Sender]),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    Worker::new(world.state.clone()).run_until_idle().await;
    assert_eq!(sends_recorded(&world).await, 0);
}

/// The outbound changes the connector recorded in workspace A.
async fn changes_recorded(world: &World) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM connectors.outbound_changes WHERE workspace_id = $1")
        .bind(world.a.as_uuid())
        .fetch_one(&world.db)
        .await
        .unwrap()
}

/// campbooks-cloud#226: `conversations.changes_requested` appended in
/// workspace A by a transaction of the runtime role, for a real
/// conversation of a connected inbox, is checked by the connector against
/// the real outbound-requests port: nothing is recorded. A member's own
/// change, through the API, is.
#[sqlx::test(migrations = "../../migrations")]
async fn a_forged_change_request_is_refused_by_the_real_port(db: PgPool) {
    use campbooks_conversations::{
        events::{ChangesRequested, RequestOrigin, RequestedChange, RequestedItem},
        model::{ConversationId, InboxId},
    };
    let world = World::new(db).await;
    let inbox = world.instances["conversations.inbox"].clone();
    let conversation = world.instances["conversations.conversation"].clone();
    // The inbox's messages stream, so the connector would record changes.
    sqlx::query(
        "INSERT INTO connectors.streams
             (id, workspace_id, connection_id, capability, phase, interval_seconds, inbox_id)
         VALUES ($1, $2, $3::uuid, 'messages', 'incremental', 60, $4::uuid)",
    )
    .bind(Uuid::now_v7())
    .bind(world.a.as_uuid())
    .bind(&world.instances["connectors.connection"])
    .bind(&inbox)
    .execute(&world.db)
    .await
    .unwrap();

    let mut tx = world.state.db.begin_for(world.a).await.unwrap();
    world
        .state
        .events
        .publish(
            &mut tx,
            &ChangesRequested {
                workspace: world.a.to_string(),
                inbox_id: inbox.parse::<InboxId>().unwrap(),
                conversation: conversation.parse::<ConversationId>().unwrap(),
                origin: RequestOrigin::User,
                items: vec![RequestedItem {
                    provider_message_id: "en-1".into(),
                    change: RequestedChange::Archive,
                }],
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    Worker::new(world.state.clone()).run_until_idle().await;
    assert_eq!(changes_recorded(&world).await, 0);

    // A member's own changes, through the flags (`request_each`), labels
    // and trash and spam operations: each one that changes something is
    // recorded.
    let token = world.token(Who::Connector.subject());
    for action in [
        "archive",
        "unarchive",
        "read",
        "unread",
        "trash",
        "untrash",
        "spam",
        "not-spam",
    ] {
        let uri = format!(
            "/api/v1/w/{}/conversations/{conversation}/{action}",
            world.a
        );
        let (status, bytes, _) = world.call(Some(&token), "POST", &uri, None).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(&bytes)
        );
    }
    Worker::new(world.state.clone()).run_until_idle().await;
    assert!(changes_recorded(&world).await > 0);
    let kinds: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT kind FROM connectors.outbound_changes WHERE workspace_id = $1 ORDER BY 1",
    )
    .bind(world.a.as_uuid())
    .fetch_all(&world.db)
    .await
    .unwrap();
    for kind in [
        "mark_read",
        "mark_unread",
        "trash",
        "untrash",
        "spam",
        "unspam",
    ] {
        assert!(
            kinds.iter().any(|k| k == kind),
            "{kind} not recorded: {kinds:?}"
        );
    }
}

/// campbooks-cloud#129: a transaction of the runtime role that sets neither
/// a workspace nor the kernel scope sees no row of any table with row-level
/// security. The tables come from `pg_class`, so a table added later is
/// covered; the world is seeded first (every kind of record of workspace A,
/// the mail it ingested, the event log, the queue, the change feed and the
/// blobs, plus instance-wide events and jobs), so the check is not vacuous.
#[sqlx::test(migrations = "../../migrations")]
async fn a_transaction_that_sets_nothing_sees_no_row_of_any_table(db: PgPool) {
    let world = World::new(db).await;
    // Instance-wide rows of the event log and the queue, and a pending
    // needs backfill, besides what the seed left.
    sqlx::query(
        "INSERT INTO kernel.events (name, version, workspace_id, payload)
         VALUES ('test.instance_wide', 1, NULL, '{}')",
    )
    .execute(&world.db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO kernel.jobs (queue, kind, payload, workspace_id, run_at, max_attempts)
         VALUES ('default', 'test.instance_wide', '{}', NULL, now() + interval '1 day', 1)",
    )
    .execute(&world.db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO conversations.needs_backfills (inbox_id, workspace_id)
         SELECT id, workspace_id FROM conversations.inboxes",
    )
    .execute(&world.db)
    .await
    .unwrap();

    // With whether the runtime role may read the table at all: one it may
    // not (the rate-limit counters, reached through the kernel's functions
    // only, campbooks-cloud#210; the web sign-in's tables, reached through
    // the BFF's, campbooks-cloud#212) must refuse the read outright.
    let listed: Vec<(String, bool)> = sqlx::query_as(
        "SELECT format('%I.%I', n.nspname, c.relname),
                has_table_privilege($1, c.oid, 'SELECT')
           FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
          WHERE c.relrowsecurity AND c.relkind IN ('r', 'p')
          ORDER BY 1",
    )
    .bind(campbooks_kernel::db::DEFAULT_RUNTIME_ROLE)
    .fetch_all(&world.db)
    .await
    .unwrap();
    let tables: Vec<String> = listed.iter().map(|(table, _)| table.clone()).collect();
    let unreadable: BTreeSet<&str> = listed
        .iter()
        .filter(|(_, readable)| !readable)
        .map(|(table, _)| table.as_str())
        .collect();
    assert_eq!(
        unreadable,
        BTreeSet::from([
            "bff.login_transactions",
            "bff.retired_tokens",
            "bff.sessions",
            "kernel.rate_limits",
        ]),
        "tables the runtime role may not read at all"
    );
    let mut populated = BTreeSet::new();
    for table in &tables {
        // The database owner (a superuser) is not subject to policies.
        let rows: i64 =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
                .fetch_one(&world.db)
                .await
                .unwrap();
        if rows > 0 {
            populated.insert(table.as_str());
        }
    }
    for sample in [
        "kernel.events",
        "kernel.jobs",
        "kernel.changes",
        "kernel.change_heads",
        "kernel.blobs",
        "kernel.blob_refs",
        "workspaces.workspaces",
        "workspaces.members",
        "workspaces.invitations",
        "conversations.inboxes",
        "conversations.messages",
        "conversations.conversations",
        "conversations.needs_backfills",
        "connectors.connections",
        "attention.items",
    ] {
        assert!(populated.contains(sample), "{sample} was not seeded");
    }

    let pool = &world.state.db;
    let mut seen = Vec::new();
    for table in &tables {
        let mut tx = pool.begin().await.unwrap();
        let counted =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
                .fetch_one(&mut *tx)
                .await;
        tx.rollback().await.unwrap();
        if unreadable.contains(table.as_str()) {
            let refused = counted.unwrap_err();
            assert!(
                refused.to_string().contains("permission denied"),
                "{table}: {refused}"
            );
            continue;
        }
        let rows: i64 = counted.unwrap();
        if rows > 0 {
            seen.push(format!("{table}: {rows}"));
        }
    }
    assert!(
        seen.is_empty(),
        "a transaction that set nothing saw rows of {seen:?} (of {} tables, {} seeded)",
        tables.len(),
        populated.len()
    );

    // Nor can it write an instance-wide row straight into the event log or
    // the queue: only the kernel's own insert, lifted, may.
    for insert in [
        "INSERT INTO kernel.jobs (queue, kind, payload, workspace_id, max_attempts)
         VALUES ('default', 'test.forged', '{}', NULL, 1)",
        "INSERT INTO kernel.events (name, version, workspace_id, payload)
         VALUES ('test.forged', 1, NULL, '{}')",
    ] {
        let mut tx = pool.begin().await.unwrap();
        let forged = sqlx::query(insert).execute(&mut *tx).await;
        assert!(forged.is_err(), "{insert}: {forged:?}");
        tx.rollback().await.unwrap();
    }

    // Not vacuous for the kernel's tables either: in the kernel scope, the
    // same pool sees every row of them, except the idempotency keys, of
    // which it sees the expired only, and the rate-limit counters, which it
    // reaches through the kernel's functions only (campbooks-cloud#210).
    let expired_keys: i64 =
        sqlx::query_scalar("SELECT count(*) FROM kernel.idempotency WHERE expires_at <= now()")
            .fetch_one(&world.db)
            .await
            .unwrap();
    let mut tx = campbooks_kernel::db::begin_kernel(pool).await.unwrap();
    for table in tables
        .iter()
        .filter(|t| t.starts_with("kernel.") && !unreadable.contains(t.as_str()))
    {
        let rows: i64 =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
                .fetch_one(&mut *tx)
                .await
                .unwrap();
        if table == "kernel.idempotency" {
            assert_eq!(rows, expired_keys, "{table}: the expired keys only");
        } else {
            assert_eq!(rows > 0, populated.contains(table.as_str()), "{table}");
        }
    }
    tx.rollback().await.unwrap();
}
