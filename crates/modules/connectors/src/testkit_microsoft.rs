//! A fake Microsoft (on wiremock): the mail tenant's token endpoint (refresh
//! only, each refresh token spent once and rotated) and Graph `/me`, for one
//! fictional account at a time. Faults are switched on one by one: a refresh
//! refused with an answer of the test's choosing, throttling, access tokens
//! that stop working.
//!
//! No test talks to Microsoft.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use url::Url;
use wiremock::{Mock, MockServer, Respond, ResponseTemplate, matchers::any};

/// The tenant the fake's mail connections are in.
pub const TENANT: &str = "common";

/// The client secret the fake accepts.
pub const CLIENT_SECRET: &str = "test-microsoft-secret";

/// The fictional work account's Graph user id.
pub const GRAPH_ID: &str = "0d9ed3e4-1c2b-4f6a-9e8d-7c6b5a4f3e2d";

/// Its primary address.
pub const ADDRESS: &str = "ana.rivera@example.com";

/// Its ID token subject (pairwise, per app).
pub const SUB: &str = "AAAAAAAAAAAAAAAAAAAAAMicrosoftSub1";

#[derive(Debug)]
struct State {
    next_id: u64,
    access_tokens: Vec<String>,
    refresh_tokens: Vec<String>,
    /// What `/me` answers.
    me: serde_json::Value,
    /// Answer the next refreshes so (status, body, `Retry-After`).
    refresh_refusal: Option<(u16, serde_json::Value, Option<u64>)>,
    /// Requests received, as `METHOD path`.
    calls: Vec<String>,
}

/// Microsoft's token endpoint and Graph `/me`, faked.
#[derive(Clone)]
pub struct FakeMicrosoft {
    server: Arc<MockServer>,
    state: Arc<Mutex<State>>,
}

struct Responder(Arc<Mutex<State>>);

impl FakeMicrosoft {
    /// A fake answering for [`GRAPH_ID`] at [`ADDRESS`].
    pub async fn start() -> Self {
        let server = MockServer::start().await;
        let state = Arc::new(Mutex::new(State {
            next_id: 1,
            access_tokens: Vec::new(),
            refresh_tokens: Vec::new(),
            me: serde_json::json!({
                "id": GRAPH_ID,
                "mail": "Ana.Rivera@Example.com",
                "userPrincipalName": "ana@example.onmicrosoft.com",
                "displayName": "Ana Rivera"
            }),
            refresh_refusal: None,
            calls: Vec::new(),
        }));
        Mock::given(any())
            .respond_with(Responder(state.clone()))
            .mount(&server)
            .await;
        Self {
            server: Arc::new(server),
            state,
        }
    }

    /// Where it is served: Microsoft's endpoints are laid out under it.
    pub fn base(&self) -> Url {
        Url::parse(&format!("{}/", self.server.uri())).unwrap()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A fresh grant's (access token, refresh token).
    pub fn grant(&self) -> (String, String) {
        let mut state = self.lock();
        let n = state.next_id;
        state.next_id += 1;
        let access = format!("fake-ms-access-{n}");
        let refresh = format!("fake-ms-refresh-{n}");
        state.access_tokens.push(access.clone());
        state.refresh_tokens.push(refresh.clone());
        (access, refresh)
    }

    /// `/me` answers `me` from now on.
    pub fn answer_me(&self, me: serde_json::Value) {
        self.lock().me = me;
    }

    /// Every refresh from now on is refused with `status` and `body`.
    pub fn refuse_refresh(&self, status: u16, body: serde_json::Value) {
        self.lock().refresh_refusal = Some((status, body, None));
    }

    /// Every refresh from now on answers 429 with `Retry-After: seconds`.
    pub fn throttle_refresh(&self, seconds: u64) {
        self.lock().refresh_refusal = Some((
            429,
            serde_json::json!({"error": "temporarily_unavailable"}),
            Some(seconds),
        ));
    }

    /// Refreshes work again.
    pub fn allow_refresh(&self) {
        self.lock().refresh_refusal = None;
    }

    /// Every access token handed out so far stops working.
    pub fn expire_access_tokens(&self) {
        self.lock().access_tokens.clear();
    }

    /// The requests received, as `METHOD path`.
    pub fn calls(&self) -> Vec<String> {
        self.lock().calls.clone()
    }

    /// How many refreshes were asked for.
    pub fn token_calls(&self) -> usize {
        self.lock()
            .calls
            .iter()
            .filter(|call| call.ends_with("/oauth2/v2.0/token"))
            .count()
    }
}

fn json(status: u16, body: &serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(body)
}

impl Respond for Responder {
    fn respond(&self, request: &wiremock::Request) -> ResponseTemplate {
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let path = request.url.path().to_owned();
        state.calls.push(format!("{} {path}", request.method));
        let token_path = format!("/{TENANT}/oauth2/v2.0/token");
        match (request.method.as_str(), path.as_str()) {
            ("POST", token) if token == token_path => refresh(&mut state, &request.body),
            ("GET", "/v1.0/me") => {
                let known = request
                    .headers
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.strip_prefix("Bearer "))
                    .is_some_and(|token| state.access_tokens.iter().any(|t| t == token));
                if !known {
                    return json(
                        401,
                        &serde_json::json!({"error": {"code": "InvalidAuthenticationToken",
                                                      "message": "Access token has expired."}}),
                    );
                }
                json(200, &state.me)
            }
            _ => ResponseTemplate::new(404),
        }
    }
}

fn refresh(state: &mut State, body: &[u8]) -> ResponseTemplate {
    let form: HashMap<String, String> = url::form_urlencoded::parse(body).into_owned().collect();
    if form.get("client_secret").map(String::as_str) != Some(CLIENT_SECRET) {
        return json(
            401,
            &serde_json::json!({"error": "invalid_client", "error_codes": [7_000_215]}),
        );
    }
    if let Some((status, answer, retry)) = state.refresh_refusal.clone() {
        let template = json(status, &answer);
        return match retry {
            Some(seconds) => template.insert_header("retry-after", seconds.to_string().as_str()),
            None => template,
        };
    }
    let known = form.get("grant_type").map(String::as_str) == Some("refresh_token")
        && form
            .get("refresh_token")
            .is_some_and(|t| state.refresh_tokens.contains(t));
    if !known {
        return json(
            400,
            &serde_json::json!({"error": "invalid_grant", "error_codes": [70000],
                                "error_description": "AADSTS70000: The provided grant is invalid."}),
        );
    }
    // The refresh token used is spent: only the rotated one works next.
    if let Some(used) = form.get("refresh_token") {
        state.refresh_tokens.retain(|t| t != used);
    }
    let n = state.next_id;
    state.next_id += 1;
    let access = format!("fake-ms-access-{n}");
    let rotated = format!("fake-ms-refresh-{n}");
    state.access_tokens.push(access.clone());
    state.refresh_tokens.push(rotated.clone());
    json(
        200,
        &serde_json::json!({
            "token_type": "Bearer",
            "access_token": access,
            "refresh_token": rotated,
            "expires_in": 3599,
            "scope": "https://graph.microsoft.com/Mail.ReadWrite https://graph.microsoft.com/Mail.Send https://graph.microsoft.com/User.Read"
        }),
    )
}
