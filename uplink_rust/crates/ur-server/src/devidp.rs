//! A built-in sign-in provider for local development and demos, so the whole login flow can run
//! without Google/GitHub/Discord keys. It is a real (tiny) OAuth2 provider with PKCE, so the
//! server's normal provider code path is what gets exercised.
//!
//! Enabled with `UPLINK_RUST_DEV_LOGIN=1`, and only when the server's public URL is loopback:
//! anyone who can reach it can sign in as anyone.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Form, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand_core::{OsRng, RngCore};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::Url;

use crate::auth::ProviderConfig;

pub const NAME: &str = "dev";
const CLIENT_ID: &str = "dev";
const CLIENT_SECRET: &str = "dev";

#[derive(Default)]
struct Inner {
    /// code -> (pkce challenge, redirect_uri, subject)
    codes: HashMap<String, (String, String, String)>,
    /// access token -> subject
    tokens: HashMap<String, String>,
}

#[derive(Clone, Default)]
pub struct DevIdp(Arc<Mutex<Inner>>);

/// True when `public_url` points at this machine.
pub fn allowed(public_url: &str) -> bool {
    Url::parse(public_url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .is_some_and(|h| h == "127.0.0.1" || h == "localhost" || h == "[::1]")
}

pub fn config(public_url: &str) -> ProviderConfig {
    let base = format!("{}/dev-idp", public_url.trim_end_matches('/'));
    ProviderConfig {
        name: NAME.into(),
        authorize_url: format!("{base}/authorize"),
        token_url: format!("{base}/token"),
        userinfo_url: format!("{base}/userinfo"),
        scopes: vec![],
        client_id: CLIENT_ID.into(),
        client_secret: CLIENT_SECRET.into(),
        subject_field: "sub",
        email_field: None,
        verified_field: None,
    }
}

fn token() -> String {
    let mut b = [0u8; 24];
    OsRng.fill_bytes(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[derive(Deserialize)]
struct AuthorizeQ {
    redirect_uri: String,
    state: String,
    code_challenge: String,
}

async fn authorize(Query(q): Query<AuthorizeQ>) -> Html<String> {
    Html(format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>Uplink sign-in (development)</title>
<style>
body{{margin:0;height:100vh;display:grid;place-items:center;background:#000;color:#b0c0f0;font:14px ui-monospace,Menlo,Consolas,monospace}}
form{{background:#00003a;border:1px solid #3050c0;padding:18px 22px;width:340px}}
h1{{font-size:15px;color:#fff;margin:0 0 10px}} p{{margin:0 0 12px;font-size:12px}}
input{{font:inherit;background:#0b1440;color:#fff;border:1px solid #5070e0;padding:4px 6px;width:calc(100% - 14px)}}
button{{margin-top:12px;font:inherit;color:#fff;background:#10208a;border:1px solid #5070e0;padding:4px 12px;cursor:pointer}}
</style></head><body><form method="get" action="approve">
<h1>Uplink account (development)</h1>
<p>This test server signs you in without Google. Pick any name; the same name always gets the same account.</p>
<input name="subject" value="agent" autofocus>
<input type="hidden" name="redirect_uri" value="{}"><input type="hidden" name="state" value="{}"><input type="hidden" name="code_challenge" value="{}">
<button type="submit">Sign in</button></form></body></html>"#,
        esc(&q.redirect_uri),
        esc(&q.state),
        esc(&q.code_challenge)
    ))
}

#[derive(Deserialize)]
struct ApproveQ {
    redirect_uri: String,
    state: String,
    code_challenge: String,
    subject: String,
}

async fn approve(State(idp): State<DevIdp>, Query(q): Query<ApproveQ>) -> Response {
    let subject = q.subject.trim();
    if subject.is_empty() || subject.len() > 64 {
        return (StatusCode::BAD_REQUEST, "pick a name").into_response();
    }
    let Ok(mut u) = Url::parse(&q.redirect_uri) else {
        return (StatusCode::BAD_REQUEST, "bad redirect_uri").into_response();
    };
    let code = token();
    idp.0.lock().unwrap().codes.insert(
        code.clone(),
        (
            q.code_challenge,
            q.redirect_uri.clone(),
            subject.to_string(),
        ),
    );
    u.query_pairs_mut()
        .append_pair("code", &code)
        .append_pair("state", &q.state);
    Redirect::to(u.as_str()).into_response()
}

#[derive(Deserialize)]
struct TokenReq {
    code: String,
    redirect_uri: String,
    client_id: String,
    client_secret: String,
    code_verifier: String,
}

async fn token_endpoint(State(idp): State<DevIdp>, Form(r): Form<TokenReq>) -> Response {
    let mut g = idp.0.lock().unwrap();
    let entry = g.codes.remove(&r.code);
    let pkce = URL_SAFE_NO_PAD.encode(Sha256::digest(r.code_verifier.as_bytes()));
    match entry {
        Some((challenge, redirect, sub))
            if r.client_id == CLIENT_ID
                && r.client_secret == CLIENT_SECRET
                && challenge == pkce
                && redirect == r.redirect_uri =>
        {
            let at = token();
            g.tokens.insert(at.clone(), sub);
            Json(serde_json::json!({ "access_token": at, "token_type": "bearer" })).into_response()
        }
        _ => StatusCode::BAD_REQUEST.into_response(),
    }
}

async fn userinfo(State(idp): State<DevIdp>, h: HeaderMap) -> Response {
    let t = h
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    match idp.0.lock().unwrap().tokens.get(t) {
        Some(sub) => Json(serde_json::json!({ "sub": sub })).into_response(),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route("/dev-idp/authorize", get(authorize))
        .route("/dev-idp/approve", get(approve))
        .route("/dev-idp/token", post(token_endpoint))
        .route("/dev-idp/userinfo", get(userinfo))
        .with_state(DevIdp::default())
}
