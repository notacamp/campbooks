//! Test fixtures: the module over a database whose connections run as the
//! runtime role, a test identity provider, stand-ins for the ports other
//! modules implement (membership, container visibility, a search source
//! holding fictional conversations in memory), in-memory blobs, and the
//! operation catalogue with a stand-in read of conversations.

use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use axum::{
    Json, Router,
    body::Body,
    http::{Method, Request, header},
};
use campbooks_kernel::{
    AppState, Error, Operation, RecordRef,
    auth::{Principal, PrincipalKind, ScopeName, Scoped},
    blobs::BlobStore,
    changes::{ContainersFuture, Visibility, VisibilityFuture},
    config::BlobsConfig,
    jobs::{Registry, Worker, WorkerStats},
    operations,
    search::{
        EntryHead, EntryText, Facet, Facets, Group, Page, Person, PersonRole, SearchSource,
        SearchSources, SourceContext, SourceFuture, facet,
    },
    tenancy::{Access, Membership, MembershipFuture, Role, UserId, WorkspaceId},
    testing::{self, TestIdp, TestResponse},
};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgPool;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::events::{CONVERSATION_RECORD, ConversationChanged};

/// The container type of the fake source.
pub const INBOX: &str = "conversations.inbox";
/// The entry type of the fake source.
pub const MESSAGE: &str = "conversations.message";

/// Every scope a member's token has.
pub const SCOPES: &str = "search:read conversations:read";

/// `conversations:read`, as the conversations module declares it.
#[derive(Debug, Clone, Copy)]
pub struct ConversationsRead;

impl ScopeName for ConversationsRead {
    const NAME: &'static str = "conversations:read";
    const DESCRIPTION: &'static str = "Read conversations (test stand-in).";
}

/// A stand-in for the conversations module's list operation: in the
/// catalogue, it says `conversations:read` reads conversations.
#[utoipa::path(
    get,
    path = "/api/v1/test/conversations",
    operation_id = "conversations.list",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["conversations.conversation"])))),
    responses((status = OK, description = "Nothing."))
)]
async fn list_conversations(_: Scoped<ConversationsRead>) -> Json<()> {
    Json(())
}

/// One message of a fake conversation.
#[derive(Debug, Clone)]
pub struct Message {
    /// Its id.
    pub id: String,
    /// The author's name and address.
    pub from: (Option<String>, String),
    /// Recipients.
    pub to: Vec<String>,
    /// Its text.
    pub text: String,
    /// File names.
    pub files: Vec<String>,
    /// `inbox`, `archive`, `spam` or `trash`.
    pub mailbox: &'static str,
    /// When it was sent.
    pub at: DateTime<Utc>,
}

impl Message {
    /// A message from `from` (`Name <address>` or an address) saying `text`,
    /// in the inbox, sent `days_ago`.
    pub fn new(id: &str, from: &str, text: &str, days_ago: i64) -> Self {
        let from = match from.split_once('<') {
            Some((name, address)) => (
                Some(name.trim().to_owned()),
                address.trim_end_matches('>').to_owned(),
            ),
            None => (None, from.to_owned()),
        };
        Self {
            id: id.to_owned(),
            from,
            to: vec!["sam@example.com".to_owned()],
            text: text.to_owned(),
            files: Vec::new(),
            mailbox: "inbox",
            at: Utc::now() - TimeDelta::days(days_ago),
        }
    }

    /// The same message carrying `files`.
    #[must_use]
    pub fn with_files(mut self, files: &[&str]) -> Self {
        self.files = files.iter().map(|&f| f.to_owned()).collect();
        self
    }

    /// The same message in `mailbox`.
    #[must_use]
    pub fn in_mailbox(mut self, mailbox: &'static str) -> Self {
        self.mailbox = mailbox;
        self
    }
}

/// A fake conversation.
#[derive(Debug, Clone)]
pub struct Conversation {
    /// Its inbox.
    pub inbox: String,
    /// Its subject.
    pub subject: String,
    /// Its stream.
    pub stream: &'static str,
    /// Unread.
    pub unread: bool,
    /// Starred.
    pub starred: bool,
    /// Label names.
    pub labels: Vec<String>,
    /// Its messages, oldest first.
    pub messages: Vec<Message>,
}

impl Conversation {
    /// A conversation in `inbox`, Primary, unread.
    pub fn new(inbox: &str, subject: &str, messages: Vec<Message>) -> Self {
        Self {
            inbox: inbox.to_owned(),
            subject: subject.to_owned(),
            stream: "primary",
            unread: true,
            starred: false,
            labels: Vec::new(),
            messages,
        }
    }
}

/// A search source over conversations held in memory, per workspace.
#[derive(Debug, Default)]
pub struct FakeSource {
    conversations: Mutex<BTreeMap<(WorkspaceId, String), Conversation>>,
    /// How many entry texts were read.
    pub texts_read: AtomicUsize,
}

impl FakeSource {
    fn group_of(&self, workspace: WorkspaceId, id: &str) -> Option<Group> {
        let conversations = self.conversations.lock().ok()?;
        let c = conversations.get(&(workspace, id.to_owned()))?;
        let facets: Facets = [
            (facet::STREAM.to_owned(), Facet::Text(c.stream.to_owned())),
            (facet::UNREAD.to_owned(), Facet::Bool(c.unread)),
            (facet::STARRED.to_owned(), Facet::Bool(c.starred)),
            (facet::ARCHIVED.to_owned(), Facet::Bool(false)),
            (
                facet::COUNT.to_owned(),
                Facet::Count(i64::try_from(c.messages.len()).unwrap_or(0)),
            ),
        ]
        .into();
        Some(Group {
            container: RecordRef::new(INBOX, &c.inbox),
            occurred_at: c
                .messages
                .iter()
                .map(|m| m.at)
                .max()
                .unwrap_or_else(Utc::now),
            title: c.subject.clone(),
            keywords: c.labels.clone(),
            facets,
            entries: c
                .messages
                .iter()
                .map(|m| EntryHead {
                    record: RecordRef::new(MESSAGE, &m.id),
                    occurred_at: m.at,
                    facets: [
                        (facet::MAILBOX.to_owned(), Facet::Text(m.mailbox.to_owned())),
                        (
                            facet::ATTACHMENTS.to_owned(),
                            Facet::Count(i64::try_from(m.files.len()).unwrap_or(0)),
                        ),
                    ]
                    .into(),
                    stamp: format!("{}|{}|{:?}", m.text, m.from.1, m.files),
                })
                .collect(),
        })
    }
}

impl SearchSource for FakeSource {
    fn group_type(&self) -> &'static str {
        CONVERSATION_RECORD
    }

    fn container_type(&self) -> &'static str {
        INBOX
    }

    fn group<'a>(&'a self, cx: SourceContext<'a>, id: &'a str) -> SourceFuture<'a, Option<Group>> {
        let group = self.group_of(cx.workspace, id);
        Box::pin(async move { Ok(group) })
    }

    fn texts<'a>(
        &'a self,
        cx: SourceContext<'a>,
        entries: &'a [RecordRef],
    ) -> SourceFuture<'a, Vec<EntryText>> {
        let texts: Vec<EntryText> = self
            .conversations
            .lock()
            .map(|conversations| {
                conversations
                    .iter()
                    .filter(|((workspace, _), _)| *workspace == cx.workspace)
                    .flat_map(|(_, c)| c.messages.iter())
                    .filter(|m| entries.iter().any(|e| e.id == m.id))
                    .map(|m| EntryText {
                        record: RecordRef::new(MESSAGE, &m.id),
                        people: std::iter::once(Person {
                            role: PersonRole::From,
                            name: m.from.0.clone(),
                            address: m.from.1.clone(),
                        })
                        .chain(m.to.iter().map(|to| Person {
                            role: PersonRole::To,
                            name: None,
                            address: to.clone(),
                        }))
                        .collect(),
                        text: m.text.clone(),
                        attachments: m.files.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.texts_read.fetch_add(texts.len(), Ordering::SeqCst);
        Box::pin(async move { Ok(texts) })
    }

    fn page<'a>(
        &'a self,
        cx: SourceContext<'a>,
        after: Option<&'a str>,
        limit: u32,
    ) -> SourceFuture<'a, Page> {
        let ids: Vec<String> = self
            .conversations
            .lock()
            .map(|conversations| {
                conversations
                    .keys()
                    .filter(|(workspace, id)| {
                        *workspace == cx.workspace && after.is_none_or(|after| id.as_str() > after)
                    })
                    .map(|(_, id)| id.clone())
                    .take(limit as usize)
                    .collect()
            })
            .unwrap_or_default();
        let next = (ids.len() == limit as usize)
            .then(|| ids.last().cloned())
            .flatten();
        Box::pin(async move { Ok(Page { groups: ids, next }) })
    }
}

/// Inbox membership, as the conversations module would answer it.
#[derive(Debug, Default)]
pub struct FakeVisibility {
    members: Mutex<HashMap<(WorkspaceId, String), Vec<UserId>>>,
}

impl FakeVisibility {
    fn visible(&self, access: &Access) -> Vec<String> {
        self.members
            .lock()
            .map(|members| {
                members
                    .iter()
                    .filter(|((workspace, _), users)| {
                        *workspace == access.workspace
                            && access.user.is_none_or(|user| users.contains(&user))
                    })
                    .map(|((_, inbox), _)| inbox.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Visibility for FakeVisibility {
    fn container_visible<'a>(
        &'a self,
        _db: &'a PgPool,
        access: &'a Access,
        container: &'a RecordRef,
    ) -> VisibilityFuture<'a> {
        let visible =
            container.record_type == INBOX && self.visible(access).contains(&container.id);
        Box::pin(async move { Ok(visible) })
    }

    fn visible_containers<'a>(
        &'a self,
        _db: &'a PgPool,
        access: &'a Access,
        container_type: &'a str,
    ) -> ContainersFuture<'a> {
        let list = if container_type == INBOX {
            self.visible(access)
                .iter()
                .map(|inbox| RecordRef::new(INBOX, inbox))
                .collect()
        } else {
            Vec::new()
        };
        Box::pin(async move { Ok(list) })
    }
}

/// The membership port: users registered with [`Kit::member`], and clients
/// bound to the workspace.
#[derive(Debug, Default)]
struct FakeMembership {
    members: Mutex<HashMap<(String, WorkspaceId), UserId>>,
}

impl Membership for FakeMembership {
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

/// The module over a fresh database.
pub struct Kit {
    /// The test database as its owner (no row-level security).
    pub db: PgPool,
    /// The state jobs and handlers run with (runtime role).
    pub state: AppState,
    /// The source.
    pub source: Arc<FakeSource>,
    app: Router,
    idp: TestIdp,
    membership: Arc<FakeMembership>,
    visibility: Arc<FakeVisibility>,
}

impl Kit {
    /// A fresh kit.
    pub async fn new(db: PgPool) -> Self {
        let idp = TestIdp::start().await;
        let pool = testing::app_role_pool(&db).await;
        let blobs = BlobStore::memory(&BlobsConfig::default(), pool.clone());
        let membership = Arc::new(FakeMembership::default());
        let visibility = Arc::new(FakeVisibility::default());
        let source = Arc::new(FakeSource::default());
        let state = idp
            .state(pool)
            .with_blobs(blobs)
            .with_membership(membership.clone())
            .with_visibility(visibility.clone())
            .with_search_sources(SearchSources::new(vec![source.clone()]))
            .with_registry(Registry::build(&[crate::module()]).unwrap());
        let api = crate::module()
            .router
            .merge(OpenApiRouter::new().routes(routes!(list_conversations)));
        let operations::Composed { routes, state, .. } = operations::compose(api, state);
        let app = routes.with_state(state.clone());
        Self {
            db,
            state,
            source,
            app,
            idp,
            membership,
            visibility,
        }
    }

    /// A user `sub`, member of `workspace`, with `scopes`.
    pub fn member_with(&self, sub: &str, workspace: WorkspaceId, scopes: &str) -> (String, UserId) {
        let user = UserId::new();
        self.membership
            .members
            .lock()
            .unwrap()
            .insert((sub.to_owned(), workspace), user);
        let mut claims = self.idp.claims(scopes);
        claims["sub"] = sub.into();
        (self.idp.token(&claims), user)
    }

    /// A user `sub`, member of `workspace`, with [`SCOPES`].
    pub fn member(&self, sub: &str, workspace: WorkspaceId) -> (String, UserId) {
        self.member_with(sub, workspace, SCOPES)
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

    /// Let `users` see `inbox` (and nobody else).
    pub fn share(&self, workspace: WorkspaceId, inbox: &str, users: &[UserId]) {
        self.visibility
            .members
            .lock()
            .unwrap()
            .insert((workspace, inbox.to_owned()), users.to_vec());
    }

    /// Store `conversation` as `id` in the source, without telling search.
    pub fn put(&self, workspace: WorkspaceId, id: &str, conversation: Conversation) {
        self.source
            .conversations
            .lock()
            .unwrap()
            .insert((workspace, id.to_owned()), conversation);
    }

    /// Change conversation `id` in the source, without telling search.
    pub fn edit(&self, workspace: WorkspaceId, id: &str, change: impl FnOnce(&mut Conversation)) {
        let mut conversations = self.source.conversations.lock().unwrap();
        if let Some(c) = conversations.get_mut(&(workspace, id.to_owned())) {
            change(c);
        }
    }

    /// Remove conversation `id` from the source, without telling search.
    pub fn remove(&self, workspace: WorkspaceId, id: &str) {
        self.source
            .conversations
            .lock()
            .unwrap()
            .remove(&(workspace, id.to_owned()));
    }

    /// Publish `conversations.conversation_changed` for `id`, as the
    /// conversations module would (not yet delivered).
    pub async fn announce(&self, workspace: WorkspaceId, id: &str) {
        let mut tx = self.state.db.begin().await.unwrap();
        self.state
            .events
            .publish(
                &mut tx,
                &ConversationChanged {
                    workspace: workspace.to_string(),
                    conversation: id.to_owned(),
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }

    /// Store, announce and index `conversation`.
    pub async fn index(&self, workspace: WorkspaceId, id: &str, conversation: Conversation) {
        self.put(workspace, id, conversation);
        self.announce(workspace, id).await;
        self.run_jobs().await;
    }

    /// Run every pending job until none is left.
    pub async fn run_jobs(&self) -> WorkerStats {
        let stats = Worker::new(self.state.clone()).run_until_idle().await;
        assert_eq!(stats.failed, 0, "{stats:?}");
        stats
    }

    /// `GET /api/v1/w/{workspace}/search?{query}`.
    pub async fn search(&self, token: &str, workspace: WorkspaceId, query: &str) -> TestResponse {
        let uri = format!("/api/v1/w/{workspace}/search?{query}");
        let mut request = Request::builder().method(Method::GET).uri(uri);
        if !token.is_empty() {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        testing::send(&self.app, request.body(Body::empty()).unwrap()).await
    }

    /// The record ids of the hits for `q` (URL-encoded by the caller).
    pub async fn ids(&self, token: &str, workspace: WorkspaceId, query: &str) -> Vec<String> {
        let response = self.search(token, workspace, query).await;
        assert_eq!(response.status, 200, "{query}: {}", response.json);
        response.json["hits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|hit| hit["record"]["id"].as_str().unwrap().to_owned())
            .collect()
    }
}
