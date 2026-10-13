//! Who sees which connection.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::{Role, WorkspaceId};
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn grantors_admins_and_clients_see_a_connection(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let bea = kit.user("bea");
    kit.join(&bea, ws, Role::Member);
    let carl = kit.user("carl");
    kit.join(&carl, ws, Role::Admin);
    let id = kit.connect(&ana, ws).await["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let list = |caller: String| {
        let kit = &kit;
        async move {
            kit.get(&caller, &format!("/api/v1/w/{ws}/connections"))
                .await
                .json["connections"]
                .as_array()
                .unwrap()
                .len()
        }
    };
    assert_eq!(list(ana.token.clone()).await, 1);
    assert_eq!(list(bea.token.clone()).await, 0);
    assert_eq!(list(carl.token.clone()).await, 1);
    assert_eq!(list(kit.client(ws).token).await, 1);

    let one = format!("/api/v1/w/{ws}/connections/{id}");
    assert_eq!(kit.get(&ana, &one).await.status, StatusCode::OK);
    assert_eq!(kit.get(&bea, &one).await.status, StatusCode::NOT_FOUND);
    assert_eq!(kit.get(&carl, &one).await.status, StatusCode::OK);
    // Another workspace's caller, or another workspace's path: 404.
    let other = WorkspaceId::new();
    kit.join(&bea, other, Role::Owner);
    assert_eq!(
        kit.get(&bea, &format!("/api/v1/w/{other}/connections/{id}"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(kit.get("", &one).await.status, StatusCode::UNAUTHORIZED);
}
