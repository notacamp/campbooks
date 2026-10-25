//! Who may do what with a page: every row of the access model, asked of the
//! central check with the module's real facts, and the module's answers to
//! the change feed and to search.

use campbooks_kernel::{
    RecordRef,
    authz::{self, Action, Decision},
    changes::Visibility as _,
    search::SourceContext,
    tenancy::{Access, Role, UserId, WorkspaceId},
};
use serde_json::json;
use sqlx::PgPool;

use crate::{
    domain::resources::{PAGE_RECORD, PageVisibility},
    testkit::{Kit, docs},
};

fn access(workspace: WorkspaceId, user: Option<UserId>, role: Option<Role>) -> Access {
    Access {
        workspace,
        user,
        role,
    }
}

async fn ask(kit: &Kit, access: &Access, action: Action, page: &str) -> Decision {
    let mut tx = kit.begin(access.workspace).await;
    let decision = authz::authorize(
        &kit.state,
        &mut tx,
        access,
        action,
        &RecordRef::new(PAGE_RECORD, page),
    )
    .await
    .unwrap();
    tx.rollback().await.unwrap();
    decision
}

async fn row(kit: &Kit, access: &Access, page: &str) -> [Decision; 5] {
    [
        ask(kit, access, Action::Read, page).await,
        ask(kit, access, Action::Write, page).await,
        ask(kit, access, Action::Create, page).await,
        ask(kit, access, Action::Share, page).await,
        ask(kit, access, Action::Delete, page).await,
    ]
}

use Decision::{Allow, Forbidden, NotFound};

fn context<'a>(
    kit: &'a Kit,
    conn: &'a mut sqlx::PgConnection,
    workspace: WorkspaceId,
) -> SourceContext<'a> {
    SourceContext {
        conn,
        blobs: &kit.state.blobs,
        workspace,
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_private_page_is_its_creators_and_whoever_it_is_shared_with(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (_, rui_id) = kit.member("rui", workspace);
    let (_, wes_id) = kit.owner("wes", workspace);
    let page = kit.add(&ana, workspace, json!({"title": "Notes"})).await;
    let id = page["id"].as_str().unwrap();
    let ana_access = access(workspace, Some(ana_id), Some(Role::Member));
    let rui_access = access(workspace, Some(rui_id), Some(Role::Member));
    let wes_access = access(workspace, Some(wes_id), Some(Role::Owner));
    let client = access(workspace, None, None);

    // Order: read, write, create, share, delete.
    assert_eq!(row(&kit, &ana_access, id).await, [Allow; 5]);
    for nobody in [&rui_access, &wes_access, &client] {
        assert_eq!(row(&kit, nobody, id).await, [NotFound; 5]);
    }

    let share = |role: &'static str| {
        let (kit, ana) = (&kit, ana.clone());
        async move {
            let granted = kit
                .put(
                    &ana,
                    &format!("{}/{id}/grants/{rui_id}", Kit::pages(workspace)),
                    json!({"role": role}),
                )
                .await;
            assert_eq!(
                granted.status,
                axum::http::StatusCode::OK,
                "{:?}",
                granted.json
            );
        }
    };
    share("reader").await;
    assert_eq!(
        row(&kit, &rui_access, id).await,
        [Allow, Forbidden, Forbidden, Forbidden, Forbidden]
    );
    share("writer").await;
    assert_eq!(
        row(&kit, &rui_access, id).await,
        [Allow, Allow, Allow, Forbidden, Forbidden]
    );
    share("manager").await;
    assert_eq!(row(&kit, &rui_access, id).await, [Allow; 5]);
    // The workspace's owner still has nothing here.
    assert_eq!(row(&kit, &wes_access, id).await, [NotFound; 5]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_workspace_page_is_every_members_and_shared_by_its_creator_and_the_owner(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let other = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (_, rui_id) = kit.member("rui", workspace);
    let (_, wes_id) = kit.owner("wes", workspace);
    let (_, bo_id) = kit.owner("bo", other);
    let page = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Handbook", "place": "workspace"}),
        )
        .await;
    let id = page["id"].as_str().unwrap();
    assert_eq!(
        row(
            &kit,
            &access(workspace, Some(ana_id), Some(Role::Member)),
            id
        )
        .await,
        [Allow; 5]
    );
    assert_eq!(
        row(
            &kit,
            &access(workspace, Some(rui_id), Some(Role::Member)),
            id
        )
        .await,
        [Allow, Allow, Allow, Forbidden, Forbidden]
    );
    assert_eq!(
        row(
            &kit,
            &access(workspace, Some(wes_id), Some(Role::Owner)),
            id
        )
        .await,
        [Allow; 5]
    );
    assert_eq!(
        row(&kit, &access(workspace, None, None), id).await,
        [NotFound; 5]
    );
    // From another workspace the page is not there, owner or not.
    let mut tx = kit.begin(other).await;
    let elsewhere = authz::authorize(
        &kit.state,
        &mut tx,
        &access(other, Some(bo_id), Some(Role::Owner)),
        Action::Read,
        &RecordRef::new(PAGE_RECORD, id),
    )
    .await
    .unwrap();
    assert_eq!(elsewhere, NotFound);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_nearest_grant_and_the_creator_of_a_page_above_decide_in_a_tree(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (_, rui_id) = kit.member("rui", workspace);
    let top = kit.add(&ana, workspace, json!({"title": "Top"})).await;
    let mid = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Mid", "parent": top["id"]}),
        )
        .await;
    let leaf = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Leaf", "parent": mid["id"]}),
        )
        .await;
    let (top, mid, leaf) = (
        top["id"].as_str().unwrap(),
        mid["id"].as_str().unwrap(),
        leaf["id"].as_str().unwrap(),
    );
    let uri = Kit::pages(workspace);
    for (page, role) in [(top, "writer"), (mid, "reader")] {
        kit.put(
            &ana,
            &format!("{uri}/{page}/grants/{rui_id}"),
            json!({"role": role}),
        )
        .await;
    }
    let rui = access(workspace, Some(rui_id), Some(Role::Member));
    // Top: writer. Mid and below: the nearer grant, reader.
    assert_eq!(row(&kit, &rui, top).await[1], Allow);
    assert_eq!(row(&kit, &rui, mid).await[1], Forbidden);
    assert_eq!(row(&kit, &rui, leaf).await[1], Forbidden);
    assert_eq!(row(&kit, &rui, leaf).await[0], Allow);
    // Ana made the top page, so she holds every page under it.
    let ana = access(workspace, Some(ana_id), Some(Role::Member));
    assert_eq!(row(&kit, &ana, leaf).await, [Allow; 5]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_feed_and_search_see_the_same_pages_the_catalogue_does(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (_, rui_id) = kit.member("rui", workspace);
    let private = kit
        .add(&ana, workspace, json!({"title": "Private plan"}))
        .await;
    let shared = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Shared plan", "place": "workspace"}),
        )
        .await;
    let private_id = private["id"].as_str().unwrap();
    let shared_id = shared["id"].as_str().unwrap();
    let ana_access = access(workspace, Some(ana_id), Some(Role::Member));
    let rui_access = access(workspace, Some(rui_id), Some(Role::Member));
    let port = PageVisibility;

    let ids = |containers: Vec<RecordRef>| {
        let mut ids: Vec<String> = containers.into_iter().map(|c| c.id).collect();
        ids.sort();
        ids
    };
    let mut expected_ana = vec![private_id.to_owned(), shared_id.to_owned()];
    expected_ana.sort();
    assert_eq!(
        ids(port
            .visible_containers(&kit.state.db, &ana_access, PAGE_RECORD)
            .await
            .unwrap()),
        expected_ana
    );
    assert_eq!(
        ids(port
            .visible_containers(&kit.state.db, &rui_access, PAGE_RECORD)
            .await
            .unwrap()),
        [shared_id]
    );
    for (who, page, seen) in [
        (&ana_access, private_id, true),
        (&rui_access, private_id, false),
        (&rui_access, shared_id, true),
    ] {
        let visible = port
            .container_visible(&kit.state.db, who, &RecordRef::new(PAGE_RECORD, page))
            .await
            .unwrap();
        assert_eq!(visible, seen);
    }
    // A client token acting for nobody sees none.
    let nobody = access(workspace, None, None);
    assert!(
        port.visible_containers(&kit.state.db, &nobody, PAGE_RECORD)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn search_reads_a_pages_title_and_text_and_a_trashed_page_leaves_the_index(db: PgPool) {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let doc = docs::client(1);
    let body = docs::paragraph(&doc, "Quarterly numbers");
    let page = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Finance", "document": STANDARD.encode(&body)}),
        )
        .await;
    let id = page["id"].as_str().unwrap().to_owned();
    let source = kit.state.search_sources.get(PAGE_RECORD).unwrap().clone();

    let mut tx = kit.begin(workspace).await;
    let group = source
        .group(context(&kit, &mut tx, workspace), &id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(group.title, "Finance");
    assert_eq!(group.container, RecordRef::new(PAGE_RECORD, &id));
    assert_eq!(group.entries.len(), 1);
    let texts = source
        .texts(
            context(&kit, &mut tx, workspace),
            std::slice::from_ref(&group.entries[0].record),
        )
        .await
        .unwrap();
    assert_eq!(texts[0].text, "Quarterly numbers\n");
    let listed = source
        .page(context(&kit, &mut tx, workspace), None, 10)
        .await
        .unwrap();
    assert_eq!(listed.groups, [id.clone()]);
    assert_eq!(listed.next, None);
    tx.rollback().await.unwrap();

    // In the trash it is no group at all; restored, it is one again.
    let uri = format!("{}/{id}", Kit::pages(workspace));
    kit.post(&ana, &format!("{uri}/trash"), None).await;
    let mut tx = kit.begin(workspace).await;
    assert!(
        source
            .group(context(&kit, &mut tx, workspace), &id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        source
            .page(context(&kit, &mut tx, workspace), None, 10)
            .await
            .unwrap()
            .groups
            .is_empty()
    );
    tx.rollback().await.unwrap();
    kit.post(&ana, &format!("{uri}/restore"), None).await;
    let mut tx = kit.begin(workspace).await;
    assert!(
        source
            .group(context(&kit, &mut tx, workspace), &id)
            .await
            .unwrap()
            .is_some()
    );
}
