#![allow(dead_code)]
//! Runs against a real Postgres. Set TEST_DATABASE_URL, e.g.
//! postgres://gl:gl@localhost/ur_test ; tests are skipped (loudly) when unset.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, Request, StatusCode};
use axum::routing::post;
use axum::{Form, Json};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use http_body_util::BodyExt;
use sha2::{Digest, Sha256};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tower::ServiceExt;
use ur_save::{seal, Key, Unlock};
use ur_server::auth::{create_account_with_session, ProviderConfig};
use ur_server::{migrate, router, AppState};
use url::Url;

pub const PUBLIC_URL: &str = "http://localhost:8080";
pub const WEB_ORIGIN: &str = "https://play.example.com";
pub const LOOPBACK: &str = "http://127.0.0.1:41234/cb";

// ---------- fake identity provider ----------

#[derive(Default)]
struct IdpInner {
    n: u32,
    /// code -> (pkce challenge, redirect_uri, subject)
    codes: HashMap<String, (String, String, String)>,
    tokens: HashMap<String, String>,
}

/// Minimal OAuth2 provider. Enforces PKCE and the client secret, like a real one.
#[derive(Clone)]
pub struct FakeIdp {
    pub base: String,
    inner: Arc<Mutex<IdpInner>>,
}

#[derive(serde::Deserialize)]
struct TokenReq {
    grant_type: String,
    code: String,
    redirect_uri: String,
    client_id: String,
    client_secret: String,
    code_verifier: String,
}

impl FakeIdp {
    pub async fn start() -> FakeIdp {
        let inner = Arc::new(Mutex::new(IdpInner::default()));
        let app = axum::Router::new()
            .route(
                "/token",
                post(|State(i): State<Arc<Mutex<IdpInner>>>, Form(r): Form<TokenReq>| async move {
                    let mut g = i.lock().unwrap();
                    let ok = r.grant_type == "authorization_code" && r.client_id == "cid" && r.client_secret == "secret";
                    let entry = g.codes.remove(&r.code); // single use
                    match entry {
                        Some((challenge, redirect, sub)) if ok && s256(&r.code_verifier) == challenge && redirect == r.redirect_uri => {
                            let at = format!("at-{}", r.code);
                            g.tokens.insert(at.clone(), sub);
                            Ok(Json(serde_json::json!({ "access_token": at, "token_type": "bearer" })))
                        }
                        _ => Err(StatusCode::BAD_REQUEST),
                    }
                }),
            )
            .route(
                "/userinfo",
                axum::routing::get(|State(i): State<Arc<Mutex<IdpInner>>>, h: HeaderMap| async move {
                    let t = h.get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).unwrap_or("");
                    match i.lock().unwrap().tokens.get(t) {
                        Some(sub) => Ok(Json(serde_json::json!({ "sub": sub, "email": format!("{sub}@example.com"), "email_verified": true }))),
                        None => Err(StatusCode::UNAUTHORIZED),
                    }
                }),
            )
            .with_state(inner.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        FakeIdp { base, inner }
    }

    pub fn config(&self, name: &str) -> ProviderConfig {
        ProviderConfig {
            name: name.into(),
            authorize_url: format!("{}/authorize", self.base),
            token_url: format!("{}/token", self.base),
            userinfo_url: format!("{}/userinfo", self.base),
            scopes: vec!["openid".into()],
            client_id: "cid".into(),
            client_secret: "secret".into(),
            subject_field: "sub",
            email_field: Some("email"),
            verified_field: Some("email_verified"),
        }
    }

    /// The user "clicks approve" on the page at `authorize_url`. Checks that our request was well-formed.
    /// Returns (code, state) that the provider would send to the callback.
    pub fn approve(&self, authorize_url: &str, subject: &str) -> (String, String) {
        let u = Url::parse(authorize_url).unwrap();
        let q: HashMap<_, _> = u.query_pairs().into_owned().collect();
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["client_id"], "cid");
        assert_eq!(q["code_challenge_method"], "S256", "PKCE must be S256");
        assert_eq!(q["code_challenge"].len(), 43);
        assert!(
            q["redirect_uri"].starts_with(PUBLIC_URL) && q["redirect_uri"].ends_with("/callback")
        );
        assert!(q["state"].len() >= 32);
        let mut g = self.inner.lock().unwrap();
        g.n += 1;
        let code = format!("code{}", g.n);
        g.codes.insert(
            code.clone(),
            (
                q["code_challenge"].clone(),
                q["redirect_uri"].clone(),
                subject.to_string(),
            ),
        );
        (code, q["state"].clone())
    }
}

pub fn s256(v: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(v.as_bytes()))
}

// ---------- app ----------

pub struct Ctx {
    pub app: axum::Router,
    pub pool: PgPool,
    pub idp: FakeIdp,
}

pub async fn ctx() -> Option<Ctx> {
    let Ok(url) = std::env::var("TEST_DATABASE_URL") else {
        eprintln!("SKIPPED: TEST_DATABASE_URL not set");
        return None;
    };
    let db = PgPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await
        .unwrap();
    migrate(&db).await.unwrap();
    let idp = FakeIdp::start().await;
    let providers = ["fake", "fake2"]
        .iter()
        .map(|n| (n.to_string(), idp.config(n)))
        .collect();
    let state = AppState::new(
        db.clone(),
        Key::from_bytes([7u8; 32]),
        providers,
        PUBLIC_URL.into(),
        vec![WEB_ORIGIN.into()],
    );
    Some(Ctx {
        app: router(state, None),
        pool: db,
        idp,
    })
}

pub async fn app() -> Option<axum::Router> {
    ctx().await.map(|c| c.app)
}

pub async fn call(app: &axum::Router, req: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
    let r = app.clone().oneshot(req).await.unwrap();
    let (p, b) = r.into_parts();
    (
        p.status,
        p.headers,
        b.collect().await.unwrap().to_bytes().to_vec(),
    )
}

pub fn rev(h: &HeaderMap) -> u64 {
    h["x-revision"].to_str().unwrap().parse().unwrap()
}

pub fn put(token: &str, slot: &str, if_rev: u64, body: Vec<u8>) -> Request<Body> {
    Request::put(format!("/v1/saves/{slot}"))
        .header("authorization", format!("Bearer {token}"))
        .header("x-if-revision", if_rev.to_string())
        .body(Body::from(body))
        .unwrap()
}

pub fn get(token: &str, path: &str) -> Request<Body> {
    Request::get(path)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

pub fn json_req(
    method: &str,
    path: &str,
    token: Option<&str>,
    body: serde_json::Value,
) -> Request<Body> {
    let mut b = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::from(body.to_string())).unwrap()
}

/// A ready-made account + session, bypassing OAuth (for tests that aren't about login).
pub async fn register(_app: &axum::Router) -> String {
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    let db = PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .unwrap();
    create_account_with_session(&db).await.unwrap().access_token
}

pub fn blob(seed: u64) -> Vec<u8> {
    seal(&ur_sim::World::new(seed), Unlock::Key(&Key::generate())).unwrap()
}
