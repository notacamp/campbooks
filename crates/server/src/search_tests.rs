//! Search end to end: the composed API with the real conversations module
//! as the source, its recorded fictional mail in four languages, the
//! workspaces module for membership, a worker running the deliveries, a real
//! Postgres (`#[sqlx::test]`) and an in-process identity provider.

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use campbooks_conversations::{dev, model::InboxRole};
use campbooks_kernel::{
    AppState,
    blobs::BlobStore,
    config::BlobsConfig,
    jobs::Worker,
    operations,
    tenancy::{UserId, WorkspaceId},
    testing::{self, TestIdp},
};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt as _;

const SCOPES: &str =
    "workspaces:read workspaces:write conversations:read conversations:write search:read";

struct Server {
    app: Router,
    state: AppState,
    idp: TestIdp,
}

impl Server {
    async fn new(db: &PgPool) -> Self {
        let idp = TestIdp::start().await;
        let mut config = testing::config("postgres://test@localhost/test");
        config.auth = idp.auth_config();
        let pool = testing::app_role_pool(db).await;
        let state = crate::with_ports(
            AppState::new(pool.clone(), config)
                .with_blobs(BlobStore::memory(&BlobsConfig::default(), pool))
                .with_registry(crate::registry().expect("valid registrations")),
        );
        let operations::Composed { routes, state, .. } = operations::compose(crate::api(), state);
        let app = routes
            .fallback(|| async { campbooks_kernel::Error::NotFound })
            .with_state(state.clone());
        Self { app, state, idp }
    }

    fn token(&self, sub: &str) -> String {
        let mut claims = self.idp.claims(SCOPES);
        claims["sub"] = sub.into();
        self.idp.token(&claims)
    }

    async fn call(
        &self,
        token: &str,
        method: &str,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {token}"));
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
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn run_jobs(&self) {
        let stats = Worker::new(self.state.clone()).run_until_idle().await;
        assert_eq!(stats.failed, 0, "{stats:?}");
    }

    async fn ingest(
        &self,
        workspace: WorkspaceId,
        inbox: campbooks_conversations::model::InboxId,
        json: &str,
    ) {
        let recorded: Value = serde_json::from_str(json).unwrap();
        let batch = dev::record(&self.state, workspace, inbox, &recorded)
            .await
            .unwrap();
        dev::submit(&self.state, workspace, &batch).await.unwrap();
        self.run_jobs().await;
    }

    async fn subjects(&self, token: &str, workspace: WorkspaceId, query: &str) -> Vec<String> {
        let uri = format!("/api/v1/w/{workspace}/search?{query}");
        let (status, body) = self.call(token, "GET", &uri, None).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        body["hits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|hit| {
                hit["title"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s["text"].as_str().unwrap())
                    .collect()
            })
            .collect()
    }
}

/// A user id of the workspaces module for `sub`, made a member of `workspace`.
async fn add_member(db: &PgPool, workspace: WorkspaceId, sub: &str) -> UserId {
    let user = UserId::new();
    sqlx::query("INSERT INTO workspaces.users (id, idp_subject) VALUES ($1, $2)")
        .bind(user.as_uuid())
        .bind(sub)
        .execute(db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspaces.members (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(workspace.as_uuid())
    .bind(user.as_uuid())
    .execute(db)
    .await
    .unwrap();
    user
}

const EN: &str = include_str!("../../modules/conversations/fixtures/en.json");
const PT: &str = include_str!("../../modules/conversations/fixtures/pt-PT.json");
const ES: &str = include_str!("../../modules/conversations/fixtures/es-ES.json");
const FR: &str = include_str!("../../modules/conversations/fixtures/fr.json");

#[sqlx::test(migrations = "../../migrations")]
async fn conversations_are_searchable_by_who_may_read_them(db: PgPool) {
    let server = Server::new(&db).await;
    let sam = server.token("sam");
    let (status, created) = server
        .call(
            &sam,
            "POST",
            "/api/v1/workspaces",
            Some(json!({"name": "Rivera Studio", "kind": "business"})),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let workspace: WorkspaceId = created["id"].as_str().unwrap().parse().unwrap();
    let sam_id: UserId = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM workspaces.users WHERE idp_subject = 'sam'",
    )
    .fetch_one(&db)
    .await
    .map(UserId::from_uuid)
    .unwrap();
    let inbox = dev::create_inbox(
        &server.state,
        workspace,
        "sam@example.com",
        &[(sam_id, InboxRole::Manager)],
    )
    .await
    .unwrap();
    for fixture in [EN, PT, ES, FR] {
        server.ingest(workspace, inbox, fixture).await;
    }

    // Accents and stems, in each language.
    assert_eq!(
        server.subjects(&sam, workspace, "q=faturas").await,
        ["A sua fatura de setembro está disponível"]
    );
    assert_eq!(
        server.subjects(&sam, workspace, "q=recu").await,
        ["Votre reçu – commande n° 3381"]
    );
    assert_eq!(
        server
            .subjects(&sam, workspace, "q=factura%20septiembre")
            .await,
        ["Factura F-2026-0457 – pedido de septiembre"]
    );
    // Operators over the real source: a sender, an attachment, a stream.
    assert_eq!(
        server
            .subjects(&sam, workspace, "q=from%3Anorthwind%20has%3Aattachment")
            .await,
        ["Q4 plan: can you review by Friday?"]
    );
    // Live: reading the conversation removes it from `is:unread`.
    let (_, page) = server
        .call(
            &sam,
            "GET",
            &format!("/api/v1/w/{workspace}/search?q=from%3Anorthwind"),
            None,
        )
        .await;
    let q4 = page["hits"][0]["record"]["id"].as_str().unwrap().to_owned();
    assert_eq!(page["hits"][0]["facets"]["unread"], true);
    let (status, _) = server
        .call(
            &sam,
            "POST",
            &format!("/api/v1/w/{workspace}/conversations/{q4}/read"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    server.run_jobs().await;
    assert!(
        server
            .subjects(&sam, workspace, "q=from%3Anorthwind%20is%3Aunread")
            .await
            .is_empty()
    );

    // A member of the workspace who is not in the inbox finds nothing.
    let rui = server.token("rui");
    add_member(&db, workspace, "rui").await;
    assert!(
        server
            .subjects(&rui, workspace, "q=fatura")
            .await
            .is_empty()
    );
    // Someone outside the workspace gets 404.
    let ana = server.token("ana");
    let (status, _) = server
        .call(
            &ana,
            "GET",
            &format!("/api/v1/w/{workspace}/search?q=fatura"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
