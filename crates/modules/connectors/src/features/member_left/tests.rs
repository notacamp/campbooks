//! A departing member's grants, and a deleted workspace's, are revoked.

use campbooks_kernel::{
    db::BeginFor as _,
    tenancy::{Role, WorkspaceId},
};
use chrono::Utc;
use sqlx::PgPool;

use crate::{
    events::{MemberLeft, WorkspaceDeleting},
    testkit::Kit,
};

async fn connections(kit: &Kit) -> i64 {
    sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM connectors.connections"#)
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_departing_members_connections_are_revoked(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    let ana_id = kit.join(&ana, ws, Role::Member);
    kit.connect(&ana, ws).await;
    let bea = kit.user("bea");
    let bea_id = kit.join(&bea, ws, Role::Member);

    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    for user in [bea_id, ana_id] {
        kit.state
            .events
            .publish(
                &mut tx,
                &MemberLeft {
                    workspace: ws.to_string(),
                    user,
                },
            )
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();
    kit.run_jobs().await;
    assert_eq!(connections(&kit).await, 0);
    assert_eq!(kit.google.revoked().len(), 1);
    assert_eq!(
        kit.payloads(ws, "connectors.connection_removed")
            .await
            .len(),
        1
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_deleted_workspaces_connections_are_revoked_at_once(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Owner);
    kit.connect(&ana, ws).await;
    let other = WorkspaceId::new();
    let bea = kit.user("bea");
    kit.join(&bea, other, Role::Owner);
    kit.connect(&bea, other).await; // a connection elsewhere stays

    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &WorkspaceDeleting {
                workspace: ws.to_string(),
                purge_after: Utc::now() + chrono::TimeDelta::days(30),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
    assert_eq!(connections(&kit).await, 1, "only the other workspace's");
    assert_eq!(kit.google.revoked().len(), 1);
}
