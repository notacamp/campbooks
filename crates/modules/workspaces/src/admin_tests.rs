//! The admin side of signups: requests, minted codes, the permission gate,
//! idempotence, atomic redemption and the security log.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use campbooks_kernel::{
    Error,
    admin::{AdminAction, AdminAudit, AdminProfiles, AuditFuture, PermissionSet, ProfilesFuture},
    config::SignupPolicy,
    testing::{self, TestResponse},
};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::testkit::{Caller, Kit};

#[derive(Debug)]
struct Profiles(HashMap<&'static str, Vec<&'static str>>);

impl AdminProfiles for Profiles {
    fn permissions_of<'a>(&'a self, _db: &'a PgPool, account: &'a str) -> ProfilesFuture<'a> {
        let names = self.0.get(account).cloned().unwrap_or_default();
        Box::pin(async move { Ok(PermissionSet::from_names(names)) })
    }
}

#[derive(Debug, Default)]
struct Trail(Mutex<Vec<AdminAction>>);

impl AdminAudit for Trail {
    fn record<'a>(&'a self, _db: &'a PgPool, action: &'a AdminAction) -> AuditFuture<'a> {
        self.0.lock().unwrap().push(action.clone());
        Box::pin(async { Ok::<(), Error>(()) })
    }
}

/// Members' routes and the admin routes over one database.
struct World {
    kit: Kit,
    admin: Router,
    trail: Arc<Trail>,
}

impl World {
    async fn new(db: PgPool, policy: SignupPolicy, env_codes: Option<&str>) -> Self {
        let kit = Kit::with_signup(db, policy, env_codes).await;
        let trail = Arc::new(Trail::default());
        let profiles = Profiles(HashMap::from([
            ("ada", vec!["super"]),
            ("sam", vec!["signups.review"]),
            ("ops", vec!["jobs.read"]),
        ]));
        let state = kit
            .state
            .clone()
            .with_admin(Arc::new(profiles), trail.clone());
        let admin = testing::admin_app(crate::module(), state);
        Self { kit, admin, trail }
    }

    async fn call(
        &self,
        caller: &Caller,
        method: Method,
        uri: &str,
        body: Option<Value>,
    ) -> TestResponse {
        let mut request = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {}", &**caller));
        let body = match body {
            Some(body) => {
                request = request.header(header::CONTENT_TYPE, "application/json");
                Body::from(body.to_string())
            }
            None => Body::empty(),
        };
        testing::send(&self.admin, request.body(body).unwrap()).await
    }

    async fn get(&self, caller: &Caller, uri: &str) -> TestResponse {
        self.call(caller, Method::GET, uri, None).await
    }

    async fn post(&self, caller: &Caller, uri: &str, body: Value) -> TestResponse {
        self.call(caller, Method::POST, uri, Some(body)).await
    }

    /// An administrator who may review signups.
    fn sam(&self) -> Caller {
        self.kit.admin_caller("sam", 60)
    }

    /// The instance's first user bootstraps it; then `sub` asks to join.
    async fn requester(&self, sub: &str) -> (Caller, String) {
        if self.kit.count_users_and_sign_ins().await.0 == 0 {
            let first = self.kit.user("first");
            self.kit.create_workspace(&first, "Ops").await;
        }
        let user = self.kit.user(sub);
        let refused = self.try_create(&user, None).await;
        assert_eq!(refused.status, StatusCode::FORBIDDEN, "{:?}", refused.json);
        let id = self.kit.user_id(sub).await.to_string();
        (user, id)
    }

    async fn try_create(&self, user: &Caller, code: Option<&str>) -> TestResponse {
        let mut body = json!({ "name": "Mine", "kind": "personal" });
        if let Some(code) = code {
            body["signup_code"] = code.into();
        }
        self.kit.post(user, "/api/v1/workspaces", body).await
    }
}

const REQUESTS: &str = "/admin/v1/workspaces/signup-requests";
const CODES: &str = "/admin/v1/workspaces/signup-codes";

#[sqlx::test(migrations = "../../../migrations")]
async fn only_signup_reviewers_get_in(db: PgPool) {
    let world = World::new(db, SignupPolicy::Approval, None).await;
    let (_, id) = world.requester("bea").await;
    let stranger = world.kit.user("bea");
    let ops = world.kit.admin_caller("ops", 60);
    let stale = world.kit.admin_caller("sam", 13 * 3600);

    // A member's token, an admin without the permission: 404, as for an
    // unknown route.
    for caller in [&stranger, &ops] {
        for (method, uri, body) in [
            (Method::GET, REQUESTS.to_owned(), None),
            (Method::POST, format!("{REQUESTS}/{id}/approve"), None),
            (Method::GET, CODES.to_owned(), None),
            (
                Method::POST,
                CODES.to_owned(),
                Some(json!({ "label": "x" })),
            ),
        ] {
            let response = world.call(caller, method, &uri, body).await;
            assert_eq!(response.status, StatusCode::NOT_FOUND, "{uri}");
        }
    }
    // An old sign-in has to be renewed.
    let response = world.get(&stale, REQUESTS).await;
    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    assert_eq!(response.json["code"], "admin_reauth_required");
    // The reviewer and a super administrator pass.
    assert_eq!(
        world.get(&world.sam(), REQUESTS).await.status,
        StatusCode::OK
    );
    let ada = world.kit.admin_caller("ada", 60);
    assert_eq!(world.get(&ada, CODES).await.status, StatusCode::OK);
    // Every passing request is on the trail.
    assert!(world.trail.0.lock().unwrap().len() >= 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn approving_lets_the_person_create_a_workspace(db: PgPool) {
    let world = World::new(db, SignupPolicy::Approval, None).await;
    let (bea, id) = world.requester("bea").await;
    // Asking again keeps one request and one log entry.
    assert_eq!(
        world.try_create(&bea, None).await.status,
        StatusCode::FORBIDDEN
    );

    let listed = world.get(&world.sam(), REQUESTS).await;
    assert_eq!(listed.status, StatusCode::OK, "{:?}", listed.json);
    let requests = listed.json["requests"].as_array().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["user_id"], id);
    assert_eq!(requests[0]["status"], "pending");
    assert_eq!(requests[0]["email"], "bea@example.com");
    assert_eq!(listed.json["next"], Value::Null);

    let approve = format!("{REQUESTS}/{id}/approve");
    let approved = world.post(&world.sam(), &approve, json!({})).await;
    assert_eq!(approved.status, StatusCode::OK, "{:?}", approved.json);
    assert_eq!(approved.json["status"], "approved");
    assert_eq!(approved.json["decided_by"], "sam");
    // Idempotent: the retry succeeds and changes nothing.
    let again = world.post(&world.sam(), &approve, json!({})).await;
    assert_eq!(again.status, StatusCode::OK);
    assert_eq!(again.json["decided_at"], approved.json["decided_at"]);

    let created = world.try_create(&bea, None).await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    // Requests, the decision (once) and the sign-in are on the log.
    assert_eq!(
        world.kit.instance_security_kinds("bea").await,
        ["signed_in", "signup_requested", "signup_approved"]
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn rejecting_keeps_refusing_with_a_clear_code(db: PgPool) {
    let world = World::new(db, SignupPolicy::Approval, None).await;
    let (bea, id) = world.requester("bea").await;
    let reject = format!("{REQUESTS}/{id}/reject");
    let rejected = world
        .post(&world.sam(), &reject, json!({ "reason": "Not a customer" }))
        .await;
    assert_eq!(rejected.status, StatusCode::OK, "{:?}", rejected.json);
    assert_eq!(rejected.json["status"], "rejected");
    assert_eq!(rejected.json["reason"], "Not a customer");
    let again = world.post(&world.sam(), &reject, json!({})).await;
    assert_eq!(again.json["reason"], "Not a customer");

    let refused = world.try_create(&bea, None).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(refused.json["code"], "signup_rejected");
    // The requester is not told why.
    assert!(!refused.json.to_string().contains("Not a customer"));
    let status = world.kit.get(&bea, "/api/v1/signup").await;
    assert_eq!(status.json["request"], "rejected");
    assert_eq!(
        world.kit.instance_security_kinds("bea").await,
        ["signed_in", "signup_requested", "signup_rejected"]
    );

    // A reviewer may change their mind.
    let approved = world
        .post(&world.sam(), &format!("{REQUESTS}/{id}/approve"), json!({}))
        .await;
    assert_eq!(approved.json["status"], "approved");
    assert_eq!(approved.json["reason"], Value::Null);
    assert_eq!(
        world.try_create(&bea, None).await.status,
        StatusCode::CREATED
    );

    let long = world
        .post(&world.sam(), &reject, json!({ "reason": "x".repeat(501) }))
        .await;
    assert_eq!(long.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn unknown_requests_are_404(db: PgPool) {
    let world = World::new(db, SignupPolicy::Approval, None).await;
    let nobody = campbooks_kernel::tenancy::UserId::new();
    for action in ["approve", "reject"] {
        let response = world
            .post(
                &world.sam(),
                &format!("{REQUESTS}/{nobody}/{action}"),
                json!({}),
            )
            .await;
        assert_eq!(response.status, StatusCode::NOT_FOUND, "{action}");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_list_is_filtered_and_paged_pending_first(db: PgPool) {
    let world = World::new(db, SignupPolicy::Approval, None).await;
    let mut ids = Vec::new();
    for sub in ["u1", "u2", "u3", "u4", "u5"] {
        ids.push(world.requester(sub).await.1);
    }
    // Decide the oldest two.
    world
        .post(
            &world.sam(),
            &format!("{REQUESTS}/{}/approve", ids[0]),
            json!({}),
        )
        .await;
    world
        .post(
            &world.sam(),
            &format!("{REQUESTS}/{}/reject", ids[1]),
            json!({}),
        )
        .await;

    let mut seen = Vec::new();
    let mut after = String::new();
    loop {
        let uri = format!("{REQUESTS}?limit=2{after}");
        let page = world.get(&world.sam(), &uri).await;
        assert_eq!(page.status, StatusCode::OK, "{:?}", page.json);
        for request in page.json["requests"].as_array().unwrap() {
            seen.push(request["user_id"].as_str().unwrap().to_owned());
        }
        match page.json["next"].as_str() {
            Some(next) => after = format!("&after={next}"),
            None => break,
        }
    }
    assert_eq!(seen.len(), 5, "{seen:?}");
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 5, "no row twice");
    // Pending (u3..u5, newest first) before the decided ones.
    assert_eq!(seen[..3], [ids[4].clone(), ids[3].clone(), ids[2].clone()]);

    let pending = world
        .get(&world.sam(), &format!("{REQUESTS}?status=pending"))
        .await;
    assert_eq!(pending.json["requests"].as_array().unwrap().len(), 3);
    let rejected = world
        .get(&world.sam(), &format!("{REQUESTS}?status=rejected"))
        .await;
    assert_eq!(rejected.json["requests"].as_array().unwrap().len(), 1);

    for bad in ["limit=0", "limit=201", "after=garbage", "status=maybe"] {
        let response = world.get(&world.sam(), &format!("{REQUESTS}?{bad}")).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{bad}");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn minted_codes_are_single_use_and_never_listed_in_clear(db: PgPool) {
    let world = World::new(db, SignupPolicy::Code, None).await;
    let first = world.kit.user("first");
    world.kit.create_workspace(&first, "Ops").await;

    let minted = world
        .post(&world.sam(), CODES, json!({ "label": "Beta, Ana" }))
        .await;
    assert_eq!(minted.status, StatusCode::CREATED, "{:?}", minted.json);
    let code = minted.json["code"].as_str().unwrap().to_owned();
    assert_eq!(minted.json["entry"]["max_uses"], 1);
    assert_eq!(minted.json["entry"]["state"], "active");

    let listed = world.get(&world.sam(), CODES).await;
    assert_eq!(listed.json["codes"].as_array().unwrap().len(), 1);
    assert!(!listed.json.to_string().contains(&code));
    let digest_hex_free = listed.json.to_string();
    assert!(!digest_hex_free.contains("digest"));

    // Typed carelessly it still works; once.
    let careless = code.to_lowercase().replace('-', " ");
    let ana = world.kit.user("ana");
    let wrong = world.try_create(&ana, Some("NOPE-NOPE")).await;
    assert_eq!(wrong.json["code"], "signup_code_invalid");
    let ok = world.try_create(&ana, Some(&careless)).await;
    assert_eq!(ok.status, StatusCode::CREATED, "{:?}", ok.json);
    let workspace = ok.json["id"].as_str().unwrap();
    assert!(
        world
            .kit
            .security_kinds(workspace)
            .await
            .contains(&"signup_code_redeemed".to_owned())
    );

    let bea = world.kit.user("bea");
    let spent = world.try_create(&bea, Some(&code)).await;
    assert_eq!(spent.status, StatusCode::FORBIDDEN);
    assert_eq!(spent.json["code"], "signup_code_invalid");

    let after = world.get(&world.sam(), CODES).await;
    assert_eq!(after.json["codes"][0]["uses"], 1);
    assert_eq!(after.json["codes"][0]["state"], "used_up");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_last_use_cannot_be_spent_twice(db: PgPool) {
    let world = World::new(db, SignupPolicy::Code, None).await;
    let first = world.kit.user("first");
    world.kit.create_workspace(&first, "Ops").await;
    let minted = world
        .post(&world.sam(), CODES, json!({ "label": "Race" }))
        .await;
    let code = minted.json["code"].as_str().unwrap().to_owned();

    let users: Vec<Caller> = (0..6)
        .map(|n| world.kit.user(&format!("racer{n}")))
        .collect();
    let attempts = futures_join(&world, &users, &code).await;
    let created = attempts
        .iter()
        .filter(|status| **status == StatusCode::CREATED)
        .count();
    assert_eq!(created, 1, "{attempts:?}");
    assert!(
        attempts
            .iter()
            .all(|s| *s == StatusCode::CREATED || *s == StatusCode::FORBIDDEN),
        "{attempts:?}"
    );
    let listed = world.get(&world.sam(), CODES).await;
    assert_eq!(listed.json["codes"][0]["uses"], 1);
}

async fn futures_join(world: &World, users: &[Caller], code: &str) -> Vec<StatusCode> {
    let mut set = tokio::task::JoinSet::new();
    for user in users {
        let kit_state = world.kit.state.clone();
        let app = testing::app(crate::module(), kit_state);
        let token = (**user).to_owned();
        let code = code.to_owned();
        set.spawn(async move {
            let request = Request::builder()
                .method(Method::POST)
                .uri("/api/v1/workspaces")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "name": "Mine", "kind": "personal", "signup_code": code }).to_string(),
                ))
                .unwrap();
            testing::send(&app, request).await.status
        });
    }
    let mut statuses = Vec::new();
    while let Some(status) = set.join_next().await {
        statuses.push(status.unwrap());
    }
    statuses
}

#[sqlx::test(migrations = "../../../migrations")]
async fn codes_can_allow_several_uses_expire_and_be_revoked(db: PgPool) {
    let world = World::new(db, SignupPolicy::Code, None).await;
    let first = world.kit.user("first");
    world.kit.create_workspace(&first, "Ops").await;
    let sam = world.sam();

    let team = world
        .post(&sam, CODES, json!({ "label": "Team", "max_uses": 2 }))
        .await;
    let team_code = team.json["code"].as_str().unwrap().to_owned();
    for sub in ["a", "b"] {
        let response = world
            .try_create(&world.kit.user(sub), Some(&team_code))
            .await;
        assert_eq!(response.status, StatusCode::CREATED, "{sub}");
    }
    let third = world
        .try_create(&world.kit.user("c"), Some(&team_code))
        .await;
    assert_eq!(third.json["code"], "signup_code_invalid");

    // Revoked codes stop working; revoking twice is fine.
    let spare = world.post(&sam, CODES, json!({ "label": "Spare" })).await;
    let spare_id = spare.json["entry"]["id"].as_str().unwrap().to_owned();
    let spare_code = spare.json["code"].as_str().unwrap().to_owned();
    for _ in 0..2 {
        let revoked = world
            .post(&sam, &format!("{CODES}/{spare_id}/revoke"), json!({}))
            .await;
        assert_eq!(revoked.status, StatusCode::OK, "{:?}", revoked.json);
        assert_eq!(revoked.json["state"], "revoked");
    }
    let refused = world
        .try_create(&world.kit.user("d"), Some(&spare_code))
        .await;
    assert_eq!(refused.json["code"], "signup_code_invalid");
    let unknown = world
        .post(
            &sam,
            &format!(
                "{CODES}/{}/revoke",
                crate::domain::model::SignupCodeId::new()
            ),
            json!({}),
        )
        .await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);

    // An expired code stops working (time passes in the database).
    let brief = world
        .post(
            &sam,
            CODES,
            json!({ "label": "Brief", "expires_at": "2999-01-01T00:00:00Z" }),
        )
        .await;
    assert_eq!(brief.status, StatusCode::CREATED, "{:?}", brief.json);
    let brief_code = brief.json["code"].as_str().unwrap().to_owned();
    sqlx::query("UPDATE workspaces.signup_codes SET expires_at = now() - interval '1 minute' WHERE label = 'Brief'")
        .execute(&world.kit.db)
        .await
        .unwrap();
    let late = world
        .try_create(&world.kit.user("e"), Some(&brief_code))
        .await;
    assert_eq!(late.json["code"], "signup_code_invalid");
    let listed = world.get(&sam, CODES).await;
    let states: Vec<&str> = listed.json["codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["state"].as_str().unwrap())
        .collect();
    assert_eq!(states, ["expired", "revoked", "used_up"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn minting_validates_its_input(db: PgPool) {
    let world = World::new(db, SignupPolicy::Code, None).await;
    for body in [
        json!({ "label": "   " }),
        json!({ "label": "x".repeat(101) }),
        json!({ "label": "ok", "max_uses": 0 }),
        json!({ "label": "ok", "max_uses": 100_001 }),
        json!({ "label": "ok", "expires_at": "2001-01-01T00:00:00Z" }),
        json!({ "label": "ok", "digest": "x" }),
    ] {
        let response = world.post(&world.sam(), CODES, body.clone()).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{body}");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn environment_codes_keep_working(db: PgPool) {
    let world = World::new(db, SignupPolicy::Code, Some("early-bird")).await;
    let first = world.kit.user("first");
    world.kit.create_workspace(&first, "Ops").await;
    // Reusable, as before.
    for sub in ["a", "b"] {
        let response = world
            .try_create(&world.kit.user(sub), Some("early-bird"))
            .await;
        assert_eq!(response.status, StatusCode::CREATED, "{sub}");
    }
    assert_eq!(
        world.get(&world.sam(), CODES).await.json["codes"],
        json!([])
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_user_cannot_read_or_decide_requests_through_the_member_api(db: PgPool) {
    // Row-level security: without an admin transaction a user sees only
    // their own request.
    let world = World::new(db, SignupPolicy::Approval, None).await;
    world.requester("bea").await;
    world.requester("cy").await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces.signup_requests")
        .fetch_one(&world.kit.state.db)
        .await
        .unwrap();
    assert_eq!(count, 0, "no user, no admin setting: no rows");
}
