//! Fixtures for the Notion sync's feature tests: a member with a Notion
//! connection, pages imported and linked, snapshots handed over as the
//! connectors module would (a blob and an event), edits made as a client
//! does, and the jobs run. No test talks to Notion.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use campbooks_kernel::tenancy::{UserId, WorkspaceId};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;
use yrs::{Text, Transact, XmlFragment, XmlOut};

use crate::{
    domain::{
        PageId,
        notion::{
            self,
            model::{SNAPSHOT_CONTENT_TYPE, Snapshot},
            tree::Tree,
        },
        store,
    },
    events::NotionPageRead,
    testkit::{Kit, docs},
};

/// A member with a Notion connection, in a workspace.
pub struct Rig {
    pub kit: Kit,
    pub ws: WorkspaceId,
    pub token: String,
    pub user: UserId,
    pub connection: Uuid,
    client: std::sync::atomic::AtomicU64,
}

impl Rig {
    /// Marta, in the Acme workspace, with her Notion connected.
    pub async fn new(db: PgPool) -> Self {
        Self::on(Kit::new(db).await)
    }

    /// The same, with tracked events written to the outbox.
    pub async fn tracking(db: PgPool) -> Self {
        Self::on(Kit::new(db).await.tracking())
    }

    fn on(kit: Kit) -> Self {
        let ws = WorkspaceId::new();
        let (token, user) = kit.member("marta", ws);
        let connection = kit.connection_of(user);
        Self {
            kit,
            ws,
            token,
            user,
            connection,
            client: std::sync::atomic::AtomicU64::new(100),
        }
    }

    /// Import Notion pages through Marta's connection.
    pub async fn import_response(
        &self,
        pages: &[(Uuid, &str)],
    ) -> campbooks_kernel::testing::TestResponse {
        self.kit
            .post(
                &self.token,
                &format!("{}/notion/imports", Kit::pages(self.ws)),
                Some(json!({
                    "connection": self.connection,
                    "pages": pages
                        .iter()
                        .map(|(page, title)| json!({"notion_page": page, "title": title}))
                        .collect::<Vec<_>>(),
                })),
            )
            .await
    }

    /// Import one Notion page: the page made.
    pub async fn import(&self, notion_page: Uuid, title: &str) -> String {
        let made = self.import_response(&[(notion_page, title)]).await;
        assert_eq!(made.status.as_u16(), 201, "{:?}", made.json);
        made.json["pages"][0]["page"].as_str().unwrap().to_owned()
    }

    /// The connector read `notion_page` and hands `snapshot` over for
    /// `page`; the jobs run.
    pub async fn read(&self, page: &str, notion_page: Uuid, snapshot: &Snapshot) {
        self.read_as(page, self.connection, notion_page, snapshot)
            .await;
    }

    /// The same, claiming a given connection.
    pub async fn read_as(
        &self,
        page: &str,
        connection: Uuid,
        notion_page: Uuid,
        snapshot: &Snapshot,
    ) {
        let bytes = serde_json::to_vec(snapshot).unwrap();
        let blob = self
            .kit
            .state
            .blobs
            .put(&self.ws.to_string(), bytes, SNAPSHOT_CONTENT_TYPE)
            .await
            .unwrap();
        let mut tx = self.kit.begin(self.ws).await;
        self.kit
            .state
            .events
            .publish(
                &mut tx,
                &NotionPageRead {
                    workspace: self.ws.to_string(),
                    connection,
                    notion_page,
                    page: page.parse().unwrap(),
                    snapshot: blob,
                    read_at: snapshot.read_at,
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        self.kit.run_jobs().await;
    }

    /// The page's body, read as blocks.
    pub async fn tree(&self, page: &str) -> Tree {
        let page: PageId = page.parse().unwrap();
        let mut tx = self.kit.begin(self.ws).await;
        let loaded = store::load(&mut tx, page).await.unwrap();
        let resolver = notion::resolver(&mut tx, &self.kit.state, self.ws, page)
            .await
            .unwrap();
        let root = loaded
            .doc
            .get_or_insert_xml_fragment(crate::domain::document::ROOT);
        let txn = loaded.doc.transact();
        notion::tree::read(&txn, &root, &resolver)
    }

    /// The text of every block of the page, in order.
    pub async fn texts(&self, page: &str) -> Vec<String> {
        self.tree(page)
            .await
            .all()
            .into_iter()
            .map(|node| node.block.text())
            .collect()
    }

    /// Marta changes the page as a client does: fetch the page, edit the
    /// copy, send the update through the sync endpoint.
    pub async fn edit_client(
        &self,
        page: &str,
        change: impl FnOnce(&mut yrs::TransactionMut<'_>, &yrs::XmlFragmentRef),
    ) {
        let client = docs::client(
            self.client
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        );
        let fetched = self
            .kit
            .sync(&self.token, self.ws, page, &client, None)
            .await;
        docs::apply(
            &client,
            &STANDARD
                .decode(fetched.json["update"].as_str().unwrap())
                .unwrap(),
        );
        let update = docs::edit(&client, change);
        let sent = self
            .kit
            .sync(&self.token, self.ws, page, &client, Some(&update))
            .await;
        assert_eq!(sent.status.as_u16(), 200, "{:?}", sent.json);
    }

    /// Marta types `more` at the end of the `at`th top-level block.
    pub async fn type_into(&self, page: &str, at: u32, more: &str) {
        self.edit_client(page, |txn, fragment| {
            let Some(XmlOut::Element(block)) = fragment.get(txn, at) else {
                panic!("no block {at}");
            };
            let Some(XmlOut::Text(text)) = block.get(txn, 0) else {
                panic!("block {at} has no text");
            };
            let end = text.len(txn);
            text.insert(txn, end, more);
        })
        .await;
    }

    /// The edits settle (the page is quiet) and the push job runs.
    pub async fn push_now(&self) {
        sqlx::query("UPDATE pages.pages SET edited_at = now() - interval '1 hour'")
            .execute(&self.kit.db)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE kernel.jobs SET run_at = now()
              WHERE kind = 'pages.push_to_notion' AND state = 'pending'",
        )
        .execute(&self.kit.db)
        .await
        .unwrap();
        self.kit.run_jobs().await;
    }

    /// A page's link row, as JSON.
    pub async fn link(&self, page: &str) -> Value {
        sqlx::query_scalar("SELECT to_jsonb(l) FROM pages.external_links l WHERE page_id = $1")
            .bind(page.parse::<Uuid>().unwrap())
            .fetch_one(&self.kit.db)
            .await
            .unwrap()
    }

    /// The requests of a page, oldest first: `(state, items)`.
    pub async fn requests(&self, page: &str) -> Vec<(Uuid, String, Value)> {
        sqlx::query_as(
            "SELECT id, state, items FROM pages.notion_requests WHERE page_id = $1 ORDER BY id",
        )
        .bind(page.parse::<Uuid>().unwrap())
        .fetch_all(&self.kit.db)
        .await
        .unwrap()
    }

    /// The bases the last sync agreed, as `(bid, notion block)`.
    pub async fn bases(&self, page: &str) -> Vec<(Uuid, Uuid)> {
        sqlx::query_as(
            "SELECT bid, notion_block_id FROM pages.external_blocks WHERE page_id = $1
              ORDER BY notion_block_id",
        )
        .bind(page.parse::<Uuid>().unwrap())
        .fetch_all(&self.kit.db)
        .await
        .unwrap()
    }
}

/// Validate `value` against the contract `file` in `contracts/`.
pub fn assert_contract(file: &str, value: &Value) {
    let path = format!("{}/../../../contracts/{file}", env!("CARGO_MANIFEST_DIR"));
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut schemas = boon::Schemas::new();
    let mut compiler = boon::Compiler::new();
    compiler.add_resource(&path, schema).unwrap();
    let index = compiler.compile(&path, &mut schemas).unwrap();
    if let Err(err) = schemas.validate(value, index) {
        panic!("{file}: {err:#}\n{value:#}");
    }
}

impl Rig {
    /// A blob of the workspace, parsed as JSON.
    pub async fn blob_json(&self, blob: &Value) -> Value {
        use futures_util::StreamExt as _;
        let blob: campbooks_kernel::blobs::BlobRef = serde_json::from_value(blob.clone()).unwrap();
        let mut stream = self
            .kit
            .state
            .blobs
            .get(&self.ws.to_string(), &blob)
            .await
            .unwrap();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            bytes.extend_from_slice(&chunk.unwrap());
        }
        serde_json::from_slice(&bytes).unwrap()
    }

    /// What the connectors module is told for `request`, as the port gives
    /// it: `None` when it does not stand.
    pub async fn stands(
        &self,
        request: Uuid,
    ) -> Option<campbooks_kernel::outbound::StoredPageChanges> {
        let mut tx = self.kit.begin(self.ws).await;
        let stored = crate::domain::notion::requests::page_requests()
            .stored_changes(&self.kit.state, &mut tx, self.ws, request)
            .await
            .unwrap();
        tx.rollback().await.unwrap();
        stored
    }
}
