//! `deploy/idp/provision.sh` against a fake Rauthy admin API.
//!
//! The fake mirrors the endpoints and payloads of Rauthy 0.36.2 that the
//! script calls (scopes, clients, client secrets and upstream sign-in
//! providers), including Rauthy's habit of echoing a provider's client secret
//! in its responses. Needs `sh` and `curl` on `PATH`; no Docker.

#[cfg(test)]
mod provision {
    use std::{
        fmt::Write as _,
        fs,
        path::{Path, PathBuf},
        process::{Command, Output},
        sync::{
            Arc, Mutex,
            atomic::{AtomicU32, Ordering},
        },
    };

    use regex::Regex;
    use serde_json::{Value, json};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate, matchers::any};

    const API_KEY: &str = "campbooks$test-api-key";
    const GOOGLE_SECRET: &str = r#"not-a-real-g00gle"quote\back"#;
    const MICROSOFT_SECRET: &str = "Ms~secret.value_with-chars";
    const ZOHO_SECRET: &str = "zoho0123456789abcdef";

    /// Rauthy's `RE_URI` and `RE_SCOPE_SPACE` (0.36.2), which validate a
    /// `ProviderRequest`.
    fn re_uri() -> Regex {
        Regex::new(r"^[a-zA-Z0-9,.:/_\-&?=~#!$'()*+%@]+$").expect("regex")
    }
    fn re_scope() -> Regex {
        Regex::new(r"^[a-zA-Z0-9\-_/:\s*]{0,512}$").expect("regex")
    }

    #[derive(Default)]
    struct State {
        /// The API key may use the `AuthProviders` group.
        provider_rights: bool,
        /// Creating a provider fails with a body that echoes the request.
        fail_create_echoing: bool,
        scopes: Vec<String>,
        clients: Vec<String>,
        /// `(id, request)` in creation order.
        providers: Vec<(String, Value)>,
        next_id: u32,
        /// `"<METHOD> <path below /auth/v1>"` for every call.
        calls: Vec<String>,
    }

    /// A provider as Rauthy serializes `ProviderResponse`: flat, `id` first,
    /// the client secret in clear text.
    fn provider_json(id: &str, p: &Value) -> String {
        let fields = [
            "name",
            "typ",
            "enabled",
            "issuer",
            "authorization_endpoint",
            "token_endpoint",
            "userinfo_endpoint",
            "jwks_endpoint",
            "client_id",
            "client_secret",
            "scope",
            "admin_claim_path",
            "admin_claim_value",
            "mfa_claim_path",
            "mfa_claim_value",
            "use_pkce",
            "client_secret_basic",
            "client_secret_post",
            "auto_onboarding",
            "auto_link",
        ];
        let mut out = format!("{{\"id\":{}", json!(id));
        for field in fields {
            let value = p.get(field).cloned().unwrap_or(Value::Null);
            let _ = write!(out, ",\"{field}\":{value}");
        }
        out.push('}');
        out
    }

    /// Rauthy's checks on a `ProviderRequest`, or the reason it is refused.
    fn invalid_provider(p: &Value) -> Option<String> {
        for field in ["name", "typ", "issuer", "client_id", "scope"] {
            if !p[field].is_string() {
                return Some(format!("{field} must be a string"));
            }
        }
        for field in [
            "issuer",
            "authorization_endpoint",
            "token_endpoint",
            "userinfo_endpoint",
            "jwks_endpoint",
            "client_id",
        ] {
            let ok = p[field].as_str().is_some_and(|v| re_uri().is_match(v));
            if !ok {
                return Some(format!("{field} is not a valid URI"));
            }
        }
        for field in [
            "enabled",
            "use_pkce",
            "client_secret_basic",
            "client_secret_post",
            "auto_onboarding",
            "auto_link",
        ] {
            if !p[field].is_boolean() {
                return Some(format!("{field} must be a boolean"));
            }
        }
        if !re_scope().is_match(p["scope"].as_str().unwrap_or_default()) {
            return Some("bad scope".into());
        }
        if !matches!(
            p["typ"].as_str(),
            Some("google" | "oidc" | "github" | "custom")
        ) {
            return Some("bad typ".into());
        }
        if p["client_secret"].is_string()
            && !(p["client_secret_basic"] == true || p["client_secret_post"] == true)
        {
            return Some("a confidential client needs an auth method".into());
        }
        None
    }

    struct FakeRauthy(Arc<Mutex<State>>);

    impl Respond for FakeRauthy {
        fn respond(&self, req: &Request) -> ResponseTemplate {
            let mut s = self.0.lock().expect("state");
            let path = req
                .url
                .path()
                .strip_prefix("/auth/v1")
                .unwrap_or_default()
                .to_owned();
            let method = req.method.as_str().to_owned();
            s.calls.push(format!("{method} {path}"));

            let authorized = req
                .headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                == Some(format!("API-Key {API_KEY}").as_str());
            if !authorized {
                return ResponseTemplate::new(401);
            }
            let body: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
            let ok = || ResponseTemplate::new(200);
            let json_body =
                |v: String| ResponseTemplate::new(200).set_body_raw(v, "application/json");

            if path.starts_with("/providers") && !s.provider_rights {
                return ResponseTemplate::new(403).set_body_raw(
                    r#"{"error":"Forbidden","message":"missing access rights"}"#,
                    "application/json",
                );
            }

            match (method.as_str(), path.as_str()) {
                ("GET", "/scopes") => {
                    let list: Vec<Value> = s
                        .scopes
                        .iter()
                        .map(|name| json!({ "id": name, "name": name }))
                        .collect();
                    json_body(Value::Array(list).to_string())
                }
                ("POST", "/scopes") => {
                    let scope = body["scope"].as_str().unwrap_or_default().to_owned();
                    s.scopes.push(scope);
                    ok()
                }
                ("POST", "/clients") => {
                    let id = body["id"].as_str().unwrap_or_default().to_owned();
                    s.clients.push(id);
                    ok()
                }
                ("POST", p) if p.starts_with("/clients/") && p.ends_with("/secret") => {
                    json_body(json!({ "id": "x", "secret": "web-client-secret" }).to_string())
                }
                ("GET" | "PUT", p) if p.starts_with("/clients/") => {
                    let id = &p["/clients/".len()..];
                    if s.clients.iter().any(|c| c == id) {
                        ok()
                    } else {
                        ResponseTemplate::new(404)
                    }
                }
                ("POST", "/providers") => {
                    let list: Vec<String> = s
                        .providers
                        .iter()
                        .map(|(id, p)| provider_json(id, p))
                        .collect();
                    json_body(format!("[{}]", list.join(",")))
                }
                ("POST", "/providers/create") => {
                    if s.fail_create_echoing {
                        return ResponseTemplate::new(400).set_body_raw(
                            format!(r#"{{"error":"BadRequest","message":"rejected {body}"}}"#),
                            "application/json",
                        );
                    }
                    if let Some(reason) = invalid_provider(&body) {
                        return ResponseTemplate::new(400).set_body_raw(
                            json!({ "error": "BadRequest", "message": reason }).to_string(),
                            "application/json",
                        );
                    }
                    s.next_id += 1;
                    let id = format!("prov{}", s.next_id);
                    let resp = provider_json(&id, &body);
                    s.providers.push((id, body));
                    json_body(resp)
                }
                ("PUT", p) if p.starts_with("/providers/") => {
                    let id = &p["/providers/".len()..];
                    if let Some(reason) = invalid_provider(&body) {
                        return ResponseTemplate::new(400).set_body_string(reason);
                    }
                    match s.providers.iter_mut().find(|(pid, _)| pid == id) {
                        Some(entry) => {
                            entry.1 = body;
                            ok()
                        }
                        None => ResponseTemplate::new(404),
                    }
                }
                _ => ResponseTemplate::new(404),
            }
        }
    }

    static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

    struct Harness {
        server: MockServer,
        state: Arc<Mutex<State>>,
        dir: PathBuf,
        env: Vec<(String, String)>,
    }

    impl Drop for Harness {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    impl Harness {
        async fn new(provider_rights: bool) -> Self {
            let state = Arc::new(Mutex::new(State {
                provider_rights,
                ..State::default()
            }));
            let server = MockServer::start().await;
            Mock::given(any())
                .respond_with(FakeRauthy(state.clone()))
                .mount(&server)
                .await;

            let dir = std::env::temp_dir().join(format!(
                "campbooks-idp-provision-{}-{}",
                std::process::id(),
                NEXT_DIR.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(dir.join("data/campbooks")).expect("data dir");
            fs::create_dir_all(dir.join("server-secrets")).expect("secrets dir");
            fs::create_dir_all(dir.join("provider-secrets")).expect("provider secrets dir");
            fs::write(dir.join("data/campbooks/api-key"), API_KEY).expect("api key");

            Self {
                server,
                state,
                dir,
                env: Vec::new(),
            }
        }

        fn set(&mut self, key: &str, value: &str) {
            self.env.retain(|(k, _)| k != key);
            self.env.push((key.to_owned(), value.to_owned()));
        }

        fn unset(&mut self, key: &str) {
            self.env.retain(|(k, _)| k != key);
        }

        fn secret_file(&self, provider: &str) -> PathBuf {
            self.dir
                .join("provider-secrets")
                .join(format!("{provider}-client-secret"))
        }

        /// Configures a provider with its client ID and secret file. The file
        /// ends with a newline, as an editor would write it.
        fn configure(&mut self, provider: &str, secret: &str) {
            let upper = provider.to_uppercase();
            let file = self.secret_file(provider);
            fs::write(&file, format!("{secret}\n")).expect("secret file");
            self.set(
                &format!("CAMPBOOKS_IDP_{upper}_CLIENT_ID"),
                &format!("{provider}-client-id.example.com"),
            );
            self.set(
                &format!("CAMPBOOKS_IDP_{upper}_CLIENT_SECRET_FILE"),
                &file.to_string_lossy(),
            );
        }

        fn marker(&self, name: &str) -> PathBuf {
            self.dir.join("data/campbooks/provisioned").join(name)
        }

        fn calls(&self) -> Vec<String> {
            self.state.lock().expect("state").calls.clone()
        }

        fn clear_calls(&self) {
            self.state.lock().expect("state").calls.clear();
        }

        fn providers(&self) -> Vec<(String, Value)> {
            self.state.lock().expect("state").providers.clone()
        }

        /// Runs the script; returns its output with stdout and stderr joined.
        async fn run(&self) -> (Output, String) {
            let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../deploy/idp/provision.sh");
            let mut cmd = Command::new("sh");
            cmd.arg(script).env_clear();
            for var in ["PATH", "TMPDIR"] {
                if let Ok(value) = std::env::var(var) {
                    cmd.env(var, value);
                }
            }
            cmd.env("IDP_INTERNAL_URL", self.server.uri())
                .env("IDP_DATA_DIR", self.dir.join("data"))
                .env("SERVER_SECRETS_DIR", self.dir.join("server-secrets"))
                .env("CAMPBOOKS_PUBLIC_URL", "https://campbooks.example.com")
                .env("CAMPBOOKS_IDP_URL", "https://auth.example.com");
            for (key, value) in &self.env {
                cmd.env(key, value);
            }
            let output = tokio::task::spawn_blocking(move || cmd.output())
                .await
                .expect("join")
                .expect("run sh (is curl on PATH?)");
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            (output, text)
        }
    }

    fn assert_no_secret(log: &str) {
        for secret in [
            GOOGLE_SECRET,
            MICROSOFT_SECRET,
            ZOHO_SECRET,
            "rotated-secret",
        ] {
            assert!(!log.contains(secret), "secret in the output:\n{log}");
        }
        // The JSON-escaped form either.
        assert!(!log.contains("g00gle"), "secret in the output:\n{log}");
    }

    fn provider_calls(calls: &[String]) -> Vec<&String> {
        calls.iter().filter(|c| c.contains("/providers")).collect()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn nothing_configured_makes_no_provider_call() {
        // An install from before this change: the API key has no provider rights.
        let h = Harness::new(false).await;
        let (out, log) = h.run().await;
        assert!(out.status.success(), "{log}");
        assert!(provider_calls(&h.calls()).is_empty(), "{:?}", h.calls());
        assert!(!log.contains("sign-in provider"), "{log}");
        assert!(
            h.dir
                .join("server-secrets/campbooks-web.client-secret")
                .exists()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn registers_each_configured_provider_once() {
        let mut h = Harness::new(true).await;
        h.configure("google", GOOGLE_SECRET);
        h.configure("microsoft", MICROSOFT_SECRET);
        h.configure("zoho", ZOHO_SECRET);
        h.set("CAMPBOOKS_IDP_MICROSOFT_TENANT", "organizations");
        h.set("CAMPBOOKS_IDP_ZOHO_ISSUER", "https://accounts.zoho.com/");

        let (out, log) = h.run().await;
        assert!(out.status.success(), "{log}");
        assert_no_secret(&log);
        assert!(
            log.contains("https://auth.example.com/auth/v1/providers/callback"),
            "{log}"
        );

        let providers = h.providers();
        assert_eq!(providers.len(), 3);
        let by_name = |name: &str| {
            providers
                .iter()
                .find(|(_, p)| p["name"] == name)
                .map(|(_, p)| p.clone())
                .expect(name)
        };
        let google = by_name("Google");
        assert_eq!(google["typ"], "google");
        assert_eq!(google["issuer"], "https://accounts.google.com");
        assert_eq!(
            google["token_endpoint"],
            "https://oauth2.googleapis.com/token"
        );
        assert_eq!(google["client_id"], "google-client-id.example.com");
        assert_eq!(
            google["client_secret"], GOOGLE_SECRET,
            "escaped and trimmed"
        );
        for p in [&google, &by_name("Microsoft"), &by_name("Zoho")] {
            assert_eq!(p["scope"], "openid email profile");
            assert_eq!(p["use_pkce"], true);
            assert_eq!(p["client_secret_post"], true);
            assert_eq!(p["client_secret_basic"], false);
            assert_eq!(p["auto_link"], false, "auto-link is a takeover path");
            assert_eq!(p["auto_onboarding"], true, "default policy is open");
            assert_eq!(p["enabled"], true);
        }
        let microsoft = by_name("Microsoft");
        assert_eq!(microsoft["typ"], "oidc");
        assert_eq!(
            microsoft["issuer"],
            "https://login.microsoftonline.com/organizations/v2.0"
        );
        assert_eq!(
            microsoft["authorization_endpoint"],
            "https://login.microsoftonline.com/organizations/oauth2/v2.0/authorize"
        );
        assert_eq!(microsoft["client_secret"], MICROSOFT_SECRET);
        let zoho = by_name("Zoho");
        assert_eq!(zoho["typ"], "oidc");
        assert_eq!(zoho["issuer"], "https://accounts.zoho.com");
        assert_eq!(
            zoho["token_endpoint"],
            "https://accounts.zoho.com/oauth/v2/token"
        );

        for key in ["google", "microsoft", "zoho"] {
            let marker = fs::read_to_string(h.marker(&format!("provider-{key}"))).expect("marker");
            assert!(marker.starts_with("id=prov"), "{marker}");
            assert!(!marker.contains("g00gle"), "{marker}");
        }

        // Second run: nothing to do.
        h.clear_calls();
        let (out, log) = h.run().await;
        assert!(out.status.success(), "{log}");
        assert!(provider_calls(&h.calls()).is_empty(), "{:?}", h.calls());
        for key in ["google", "microsoft", "zoho"] {
            assert!(
                log.contains(&format!("sign-in provider {key} already provisioned")),
                "{log}"
            );
        }
        assert_eq!(h.providers().len(), 3);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn admin_edits_survive_until_the_settings_change() {
        let mut h = Harness::new(true).await;
        h.configure("google", GOOGLE_SECRET);
        let (out, log) = h.run().await;
        assert!(out.status.success(), "{log}");

        // An admin renames the provider in the admin UI.
        h.state.lock().expect("state").providers[0].1["name"] = json!("Google Workspace");
        let (out, log) = h.run().await;
        assert!(out.status.success(), "{log}");
        assert_eq!(h.providers()[0].1["name"], "Google Workspace");

        // The secret is rotated: the provider is re-applied, with the new secret.
        fs::write(h.secret_file("google"), "rotated-secret").expect("rotate");
        h.clear_calls();
        let (out, log) = h.run().await;
        assert!(out.status.success(), "{log}");
        assert_no_secret(&log);
        assert!(log.contains("re-applied sign-in provider google"), "{log}");
        assert_eq!(
            h.calls()
                .iter()
                .filter(|c| c.starts_with("PUT /providers/"))
                .count(),
            1
        );
        let providers = h.providers();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].1["client_secret"], "rotated-secret");
        assert_eq!(providers[0].1["name"], "Google");

        // The signup policy tightens: new accounts are switched off.
        h.set("CAMPBOOKS_SIGNUP__POLICY", "invite_only");
        let (out, log) = h.run().await;
        assert!(out.status.success(), "{log}");
        assert_eq!(h.providers()[0].1["auto_onboarding"], false);

        // Deleted in the admin UI while its settings changed: created again.
        h.state.lock().expect("state").providers.clear();
        h.set("CAMPBOOKS_SIGNUP__POLICY", "open");
        let (out, log) = h.run().await;
        assert!(out.status.success(), "{log}");
        assert!(log.contains("created sign-in provider google"), "{log}");
        assert_eq!(h.providers().len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn signup_policy_decides_whether_providers_create_accounts() {
        let cases = [
            ("open", None, true),
            ("code", None, true),
            ("approval", None, true),
            ("invite_only", None, false),
            ("invite_only", Some("open"), true),
            ("open", Some("closed"), false),
            ("invite_only", Some("auto"), false),
        ];
        for (policy, signup, expected) in cases {
            let mut h = Harness::new(true).await;
            h.configure("zoho", ZOHO_SECRET);
            h.set("CAMPBOOKS_SIGNUP__POLICY", policy);
            if let Some(signup) = signup {
                h.set("CAMPBOOKS_IDP_PROVIDER_SIGNUP", signup);
            }
            let (out, log) = h.run().await;
            assert!(out.status.success(), "{policy} {signup:?}: {log}");
            assert_eq!(
                h.providers()[0].1["auto_onboarding"],
                expected,
                "{policy} {signup:?}"
            );
            assert!(
                log.contains(&format!("may create accounts for new people: {expected}")),
                "{log}"
            );
        }

        for (key, value) in [
            ("CAMPBOOKS_IDP_PROVIDER_SIGNUP", "sometimes"),
            ("CAMPBOOKS_SIGNUP__POLICY", "everyone"),
        ] {
            let mut h = Harness::new(true).await;
            h.configure("zoho", ZOHO_SECRET);
            h.set(key, value);
            let (out, log) = h.run().await;
            assert!(!out.status.success(), "{log}");
            assert!(log.contains(key), "{log}");
            assert!(provider_calls(&h.calls()).is_empty());
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn adopts_a_provider_with_the_same_issuer() {
        let mut h = Harness::new(true).await;
        h.state.lock().expect("state").providers.push((
            "byhand".into(),
            json!({ "name": "Google", "issuer": "https://accounts.google.com", "client_secret": "old" }),
        ));
        h.configure("google", GOOGLE_SECRET);
        let (out, log) = h.run().await;
        assert!(out.status.success(), "{log}");
        assert_no_secret(&log);
        assert!(
            log.contains("configured existing sign-in provider google"),
            "{log}"
        );
        let calls = h.calls();
        assert!(
            calls.contains(&"PUT /providers/byhand".to_owned()),
            "{calls:?}"
        );
        assert!(
            !calls.contains(&"POST /providers/create".to_owned()),
            "{calls:?}"
        );
        assert_eq!(h.providers().len(), 1);
        let marker = fs::read_to_string(h.marker("provider-google")).expect("marker");
        assert!(marker.starts_with("id=byhand\n"), "{marker}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn missing_rights_fail_after_clients_with_instructions() {
        let mut h = Harness::new(false).await;
        h.configure("google", GOOGLE_SECRET);
        let (out, log) = h.run().await;
        assert!(!out.status.success(), "{log}");
        assert_no_secret(&log);
        assert!(log.contains("API Keys -> campbooks"), "{log}");
        assert!(
            log.contains("https://auth.example.com/auth/v1/admin"),
            "{log}"
        );
        assert!(
            h.dir
                .join("server-secrets/campbooks-web.client-secret")
                .exists()
        );
        assert!(h.marker("campbooks-native").exists());
        assert!(!h.marker("provider-google").exists());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bad_secret_files_fail_before_any_provider_call() {
        let mut h = Harness::new(true).await;
        h.configure("microsoft", MICROSOFT_SECRET);
        fs::remove_file(h.secret_file("microsoft")).expect("remove");
        let (out, log) = h.run().await;
        assert!(!out.status.success(), "{log}");
        assert!(log.contains("no client secret file"), "{log}");
        assert!(provider_calls(&h.calls()).is_empty());

        fs::write(h.secret_file("microsoft"), "\n").expect("empty");
        let (out, log) = h.run().await;
        assert!(!out.status.success(), "{log}");
        assert!(log.contains("is empty"), "{log}");

        h.unset("CAMPBOOKS_IDP_MICROSOFT_CLIENT_SECRET_FILE");
        fs::write(h.secret_file("microsoft"), MICROSOFT_SECRET).expect("write");
        let (out, log) = h.run().await;
        assert!(!out.status.success(), "{log}");
        assert!(
            log.contains("/run/secrets/campbooks/microsoft-client-secret"),
            "the default path is the compose mount: {log}"
        );
        assert!(provider_calls(&h.calls()).is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_error_echoing_the_secret_is_withheld() {
        let mut h = Harness::new(true).await;
        h.state.lock().expect("state").fail_create_echoing = true;
        h.configure("google", GOOGLE_SECRET);
        let (out, log) = h.run().await;
        assert!(!out.status.success(), "{log}");
        assert!(log.contains("response withheld"), "{log}");
        assert_no_secret(&log);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_removed_provider_stays_and_is_reported() {
        let mut h = Harness::new(true).await;
        h.configure("zoho", ZOHO_SECRET);
        let (out, log) = h.run().await;
        assert!(out.status.success(), "{log}");

        h.unset("CAMPBOOKS_IDP_ZOHO_CLIENT_ID");
        h.clear_calls();
        let (out, log) = h.run().await;
        assert!(out.status.success(), "{log}");
        assert!(log.contains("zoho is no longer configured"), "{log}");
        assert!(provider_calls(&h.calls()).is_empty());
        assert_eq!(h.providers().len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bad_tenant_and_issuer_are_refused() {
        for (key, value) in [
            ("CAMPBOOKS_IDP_MICROSOFT_TENANT", "common/../x"),
            ("CAMPBOOKS_IDP_ZOHO_ISSUER", "http://accounts.zoho.eu"),
            (
                "CAMPBOOKS_IDP_ZOHO_ISSUER",
                "https://accounts.zoho.eu/oauth",
            ),
        ] {
            let mut h = Harness::new(true).await;
            h.configure("microsoft", MICROSOFT_SECRET);
            h.configure("zoho", ZOHO_SECRET);
            h.set(key, value);
            let (out, log) = h.run().await;
            assert!(!out.status.success(), "{key}={value}: {log}");
            assert!(log.contains(key), "{log}");
        }
    }
}
