mod common;
use std::collections::HashMap;

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use common::*;
use serde_json::{json, Value};
use url::Url;

fn uniq(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{prefix}-{t}-{}", N.fetch_add(1, Ordering::Relaxed))
}

fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// (client secret, its challenge)
fn pair() -> (String, String) {
    let v = uniq("verifier");
    let c = s256(&v);
    (v, c)
}

fn location(h: &HeaderMap) -> String {
    h["location"].to_str().unwrap().to_string()
}

fn query_of(loc: &str) -> HashMap<String, String> {
    Url::parse(loc)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}

async fn start_login(
    c: &Ctx,
    provider: &str,
    return_to: &str,
    challenge: &str,
) -> (StatusCode, HeaderMap) {
    let path = format!(
        "/v1/auth/{provider}/start?return_to={}&client_challenge={challenge}",
        enc(return_to)
    );
    let (st, h, _) = call(&c.app, Request::get(path).body(Body::empty()).unwrap()).await;
    (st, h)
}

async fn callback(c: &Ctx, provider: &str, code: &str, state: &str) -> (StatusCode, HeaderMap) {
    let (st, h, _) = call(
        &c.app,
        Request::get(format!(
            "/v1/auth/{provider}/callback?code={code}&state={state}"
        ))
        .body(Body::empty())
        .unwrap(),
    )
    .await;
    (st, h)
}

async fn exchange(c: &Ctx, login_code: &str, verifier: &str) -> (StatusCode, Value) {
    let (st, _, b) = call(
        &c.app,
        json_req(
            "POST",
            "/v1/auth/exchange",
            None,
            json!({ "login_code": login_code, "client_verifier": verifier }),
        ),
    )
    .await;
    (st, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

/// Full login for provider-side user `sub`. Returns our tokens.
async fn login_on(c: &Ctx, provider: &str, sub: &str) -> Value {
    let (ver, ch) = pair();
    let (st, h) = start_login(c, provider, LOOPBACK, &ch).await;
    assert_eq!(st, StatusCode::SEE_OTHER);
    let (code, state) = c.idp.approve(&location(&h), sub);
    let (st, h) = callback(c, provider, &code, &state).await;
    assert_eq!(st, StatusCode::SEE_OTHER);
    let loc = location(&h);
    assert!(loc.starts_with(LOOPBACK), "{loc}");
    let q = query_of(&loc);
    let (st, tokens) = exchange(c, &q["login_code"], &ver).await;
    assert_eq!(st, StatusCode::OK, "{tokens}");
    tokens
}

async fn login(c: &Ctx, sub: &str) -> Value {
    login_on(c, "fake", sub).await
}

fn tok(t: &Value) -> &str {
    t["access_token"].as_str().unwrap()
}

async fn me(c: &Ctx, t: &Value) -> (StatusCode, Value) {
    let (st, _, b) = call(&c.app, get(tok(t), "/v1/me")).await;
    (st, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

#[tokio::test]
async fn login_creates_one_account_per_identity() {
    let Some(c) = ctx().await else { return };
    let (a, b) = (uniq("alice"), uniq("bob"));
    let t1 = login(&c, &a).await;
    let t2 = login(&c, &a).await;
    let t3 = login(&c, &b).await;
    assert_eq!(
        t1["account_id"], t2["account_id"],
        "same provider identity -> same account"
    );
    assert_ne!(t1["account_id"], t3["account_id"]);
    assert_eq!(t1["needs_handle"], true);
    let (st, m) = me(&c, &t1).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(m["identities"][0]["provider"], "fake");
    assert_eq!(m["identities"][0]["email"], format!("{a}@example.com"));
}

#[tokio::test]
async fn handles_are_unique_case_insensitive_validated_and_set_once() {
    let Some(c) = ctx().await else { return };
    let (t1, t2) = (login(&c, &uniq("a")).await, login(&c, &uniq("b")).await);
    let nick = format!(
        "Gh{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    );
    let set =
        |t: &Value, h: &str| json_req("PUT", "/v1/me/handle", Some(tok(t)), json!({ "handle": h }));

    assert_eq!(call(&c.app, set(&t1, &nick)).await.0, StatusCode::OK);
    assert_eq!(
        call(&c.app, set(&t2, &nick.to_lowercase())).await.0,
        StatusCode::CONFLICT,
        "case-insensitive"
    );
    assert_eq!(
        call(&c.app, set(&t1, "another_one")).await.0,
        StatusCode::CONFLICT,
        "set once"
    );
    for bad in [
        "ab",
        "has space",
        "x".repeat(21).as_str(),
        "admin",
        "Admin",
        "emoji\u{1F600}",
        "semi;colon",
    ] {
        assert_eq!(
            call(&c.app, set(&t2, bad)).await.0,
            StatusCode::BAD_REQUEST,
            "{bad}"
        );
    }
    let (_, m) = me(&c, &t1).await;
    assert_eq!(m["handle"], nick);
    assert_eq!(m["needs_handle"], false);
}

#[tokio::test]
async fn return_to_is_an_allowlist() {
    let Some(c) = ctx().await else { return };
    let (_, ch) = pair();
    for ok in [
        WEB_ORIGIN.to_string() + "/",
        format!("{WEB_ORIGIN}/play?x=1"),
        "http://127.0.0.1:5000/cb".into(),
        "http://localhost:9/".into(),
    ] {
        assert_eq!(
            start_login(&c, "fake", &ok, &ch).await.0,
            StatusCode::SEE_OTHER,
            "{ok}"
        );
    }
    for bad in [
        "https://evil.example/",
        "https://play.example.com.evil.com/",
        "https://play.example.com:444/",
        "https://user@play.example.com/",
        "http://play.example.com/", // wrong scheme for the allowed origin
        "javascript:alert(1)",
        "//evil.example",
        "https://127.0.0.1/", // loopback is http-only
        "",
    ] {
        assert_eq!(
            start_login(&c, "fake", bad, &ch).await.0,
            StatusCode::BAD_REQUEST,
            "{bad}"
        );
    }
    assert_eq!(
        start_login(&c, "nope", LOOPBACK, &ch).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        start_login(&c, "fake", LOOPBACK, "short").await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn login_code_needs_the_clients_secret_and_is_single_use() {
    let Some(c) = ctx().await else { return };
    let (ver, ch) = pair();
    let (_, h) = start_login(&c, "fake", LOOPBACK, &ch).await;
    let (code, state) = c.idp.approve(&location(&h), &uniq("u"));
    let (_, h) = callback(&c, "fake", &code, &state).await;
    let login_code = query_of(&location(&h))["login_code"].clone();

    // An interceptor who sees the redirect URL but not the client's secret.
    assert_eq!(
        exchange(&c, &login_code, "guess").await.0,
        StatusCode::UNAUTHORIZED
    );
    // ...and the failed attempt burned the code, so the real client is locked out too (fail closed).
    assert_eq!(
        exchange(&c, &login_code, &ver).await.0,
        StatusCode::UNAUTHORIZED
    );

    // Happy path, then replay.
    let (ver, ch) = pair();
    let (_, h) = start_login(&c, "fake", LOOPBACK, &ch).await;
    let (code, state) = c.idp.approve(&location(&h), &uniq("u"));
    let (_, h) = callback(&c, "fake", &code, &state).await;
    let lc = query_of(&location(&h))["login_code"].clone();
    assert_eq!(exchange(&c, &lc, &ver).await.0, StatusCode::OK);
    assert_eq!(exchange(&c, &lc, &ver).await.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn state_is_single_use_provider_bound_and_expires() {
    let Some(c) = ctx().await else { return };
    let (_, ch) = pair();
    let (_, h) = start_login(&c, "fake", LOOPBACK, &ch).await;
    let (code, state) = c.idp.approve(&location(&h), &uniq("u"));
    assert_eq!(
        callback(&c, "fake", &code, &state).await.0,
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        callback(&c, "fake", &code, &state).await.0,
        StatusCode::BAD_REQUEST,
        "replay"
    );
    assert_eq!(
        callback(&c, "fake", "x", "unknown-state").await.0,
        StatusCode::BAD_REQUEST
    );

    // State minted for one provider can't be completed on another.
    let (_, h) = start_login(&c, "fake", LOOPBACK, &ch).await;
    let (code, state) = c.idp.approve(&location(&h), &uniq("u"));
    assert_eq!(
        callback(&c, "fake2", &code, &state).await.0,
        StatusCode::BAD_REQUEST
    );

    // Expired.
    let (_, h) = start_login(&c, "fake", LOOPBACK, &ch).await;
    let (code, state) = c.idp.approve(&location(&h), &uniq("u"));
    use sha2::{Digest, Sha256};
    sqlx::query(
        "UPDATE oauth_states SET created_at = now() - interval '11 minutes' WHERE state_hash = $1",
    )
    .bind(Sha256::digest(state.as_bytes()).to_vec())
    .execute(&c.pool)
    .await
    .unwrap();
    assert_eq!(
        callback(&c, "fake", &code, &state).await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn provider_denial_and_failure_redirect_with_a_fixed_error() {
    let Some(c) = ctx().await else { return };
    let (_, ch) = pair();
    // User clicked "deny".
    let (_, h) = start_login(&c, "fake", LOOPBACK, &ch).await;
    let state = query_of(&location(&h))["state"].clone();
    let (st, h) = call_get(
        &c,
        &format!("/v1/auth/fake/callback?error=access_denied%3Cscript%3E&state={state}"),
    )
    .await;
    assert_eq!(st, StatusCode::SEE_OTHER);
    let q = query_of(&location(&h));
    assert_eq!(
        q["error"], "access_denied",
        "provider-supplied text is never echoed"
    );
    assert!(!q.contains_key("login_code"));

    // Bogus code: the token exchange fails.
    let (_, h) = start_login(&c, "fake", LOOPBACK, &ch).await;
    let state = query_of(&location(&h))["state"].clone();
    let (_, h) = callback(&c, "fake", "forged-code", &state).await;
    assert_eq!(query_of(&location(&h))["error"], "provider_failed");
}

async fn call_get(c: &Ctx, path: &str) -> (StatusCode, HeaderMap) {
    let (st, h, _) = call(&c.app, Request::get(path).body(Body::empty()).unwrap()).await;
    (st, h)
}

#[tokio::test]
async fn pkce_verifier_is_bound_to_the_provider_request() {
    let Some(c) = ctx().await else { return };
    // The fake provider only issues a token if SHA256(code_verifier) matches the challenge we sent
    // in the authorize URL, so a successful login proves the server sent a matching pair.
    let t = login(&c, &uniq("pkce")).await;
    assert_eq!(me(&c, &t).await.0, StatusCode::OK);
}

#[tokio::test]
async fn refresh_rotates_and_detects_reuse() {
    let Some(c) = ctx().await else { return };
    let t1 = login(&c, &uniq("r")).await;
    let r1 = t1["refresh_token"].as_str().unwrap();
    let refresh = |r: &str| {
        json_req(
            "POST",
            "/v1/auth/refresh",
            None,
            json!({ "refresh_token": r }),
        )
    };

    let (st, _, b) = call(&c.app, refresh(r1)).await;
    assert_eq!(st, StatusCode::OK);
    let t2: Value = serde_json::from_slice(&b).unwrap();
    assert_ne!(t2["access_token"], t1["access_token"]);
    assert_eq!(
        me(&c, &t1).await.0,
        StatusCode::UNAUTHORIZED,
        "old access token dies on rotation"
    );
    assert_eq!(me(&c, &t2).await.0, StatusCode::OK);

    // Presenting the already-rotated refresh token again: treat as theft, kill the session.
    assert_eq!(call(&c.app, refresh(r1)).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(me(&c, &t2).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        call(&c.app, refresh(t2["refresh_token"].as_str().unwrap()))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        call(&c.app, refresh("garbage")).await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn access_tokens_expire_but_refresh_recovers() {
    let Some(c) = ctx().await else { return };
    let t = login(&c, &uniq("e")).await;
    sqlx::query("UPDATE sessions SET access_expires = now() - interval '1 second' WHERE account_id = $1::uuid")
        .bind(t["account_id"].as_str().unwrap())
        .execute(&c.pool)
        .await
        .unwrap();
    assert_eq!(me(&c, &t).await.0, StatusCode::UNAUTHORIZED);
    let (st, _, b) = call(
        &c.app,
        json_req(
            "POST",
            "/v1/auth/refresh",
            None,
            json!({ "refresh_token": t["refresh_token"] }),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let t2: Value = serde_json::from_slice(&b).unwrap();
    assert_eq!(me(&c, &t2).await.0, StatusCode::OK);
}

#[tokio::test]
async fn logout_revokes_the_session() {
    let Some(c) = ctx().await else { return };
    let t = login(&c, &uniq("l")).await;
    let req = Request::post("/v1/auth/logout")
        .header("authorization", format!("Bearer {}", tok(&t)))
        .body(Body::empty())
        .unwrap();
    assert_eq!(call(&c.app, req).await.0, StatusCode::NO_CONTENT);
    assert_eq!(me(&c, &t).await.0, StatusCode::UNAUTHORIZED);
    let (st, ..) = call(
        &c.app,
        json_req(
            "POST",
            "/v1/auth/refresh",
            None,
            json!({ "refresh_token": t["refresh_token"] }),
        ),
    )
    .await;
    assert_eq!(
        st,
        StatusCode::UNAUTHORIZED,
        "refresh token dies with the session"
    );
}

#[tokio::test]
async fn linking_unlinking_and_never_merging_accounts() {
    let Some(c) = ctx().await else { return };
    let (sub_a, sub_a2, sub_b) = (uniq("a"), uniq("a-second"), uniq("b"));
    let a = login(&c, &sub_a).await;
    let b = login(&c, &sub_b).await;

    let link = |c: &Ctx, t: Value, provider: &'static str, sub: String| {
        let c_app = c.app.clone();
        let idp = c.idp.clone();
        async move {
            let (_ver, ch) = pair();
            let body = json!({ "return_to": LOOPBACK, "client_challenge": ch });
            let (st, _, bytes) = call(
                &c_app,
                json_req(
                    "POST",
                    &format!("/v1/auth/link/{provider}"),
                    Some(tok(&t)),
                    body,
                ),
            )
            .await;
            assert_eq!(st, StatusCode::OK);
            let url = serde_json::from_slice::<Value>(&bytes).unwrap()["url"]
                .as_str()
                .unwrap()
                .to_string();
            let (code, state) = idp.approve(&url, &sub);
            let (st, h, _) = call(
                &c_app,
                Request::get(format!(
                    "/v1/auth/{provider}/callback?code={code}&state={state}"
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await;
            assert_eq!(st, StatusCode::SEE_OTHER);
            query_of(h["location"].to_str().unwrap())
        }
    };

    // A adds a second provider.
    let q = link(&c, a.clone(), "fake2", uniq("a-on-fake2")).await;
    assert_eq!(q["linked"], "fake2");
    assert!(!q.contains_key("login_code"), "linking never mints a login");
    let (_, m) = me(&c, &a).await;
    assert_eq!(m["identities"].as_array().unwrap().len(), 2);

    // B tries to attach a provider identity that already belongs to A. No merge, no theft.
    let a_fake2_sub = {
        let (_, m) = me(&c, &a).await;
        assert_eq!(m["identities"][1]["provider"], "fake2");
        m["identities"][1]["email"]
            .as_str()
            .unwrap()
            .trim_end_matches("@example.com")
            .to_string()
    };
    let q = link(&c, b.clone(), "fake2", a_fake2_sub).await;
    assert_eq!(q["error"], "identity_in_use");
    assert_eq!(
        me(&c, &b).await.1["identities"].as_array().unwrap().len(),
        1
    );

    // A second, different login on a provider A already has is refused too.
    let q = link(&c, a.clone(), "fake", sub_a2).await;
    assert_eq!(q["error"], "identity_in_use");

    // Unlink: allowed while another login remains, refused for the last one.
    let del = |t: &Value, p: &str| {
        Request::delete(format!("/v1/me/identities/{p}"))
            .header("authorization", format!("Bearer {}", tok(t)))
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(
        call(&c.app, del(&a, "fake2")).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&c.app, del(&a, "fake")).await.0,
        StatusCode::CONFLICT,
        "can't lock yourself out"
    );
    assert_eq!(call(&c.app, del(&a, "fake2")).await.0, StatusCode::CONFLICT);
}

#[tokio::test]
async fn link_requires_a_session() {
    let Some(c) = ctx().await else { return };
    let (_, ch) = pair();
    let (st, ..) = call(
        &c.app,
        json_req(
            "POST",
            "/v1/auth/link/fake",
            None,
            json!({ "return_to": LOOPBACK, "client_challenge": ch }),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn simultaneous_first_logins_make_one_account() {
    let Some(c) = ctx().await else { return };
    let sub = uniq("race");
    let mut flows = vec![];
    for _ in 0..6 {
        let (ver, ch) = pair();
        let (_, h) = start_login(&c, "fake", LOOPBACK, &ch).await;
        let (code, state) = c.idp.approve(&location(&h), &sub);
        flows.push((ver, code, state));
    }
    let mut set = tokio::task::JoinSet::new();
    for (ver, code, state) in flows {
        let app = c.app.clone();
        set.spawn(async move {
            let (st, h, _) = call(
                &app,
                Request::get(format!("/v1/auth/fake/callback?code={code}&state={state}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
            assert_eq!(st, StatusCode::SEE_OTHER);
            (
                ver,
                query_of(h["location"].to_str().unwrap())["login_code"].clone(),
            )
        });
    }
    let mut accounts = std::collections::HashSet::new();
    while let Some(r) = set.join_next().await {
        let (ver, lc) = r.unwrap();
        let (st, t) = exchange(&c, &lc, &ver).await;
        assert_eq!(st, StatusCode::OK);
        accounts.insert(t["account_id"].as_str().unwrap().to_string());
    }
    assert_eq!(accounts.len(), 1);
}

#[tokio::test]
async fn deleting_an_account_erases_everything() {
    let Some(c) = ctx().await else { return };
    let sub = uniq("del");
    let t = login(&c, &sub).await;
    // Give it some data.
    let (st, ..) = call(&c.app, put(tok(&t), "main", 0, blob(1))).await;
    assert_eq!(st, StatusCode::OK);
    let req = Request::post("/v1/seeds")
        .header("authorization", format!("Bearer {}", tok(&t)))
        .body(Body::empty())
        .unwrap();
    assert_eq!(call(&c.app, req).await.0, StatusCode::OK);

    let del = |confirm: &str| {
        json_req(
            "DELETE",
            "/v1/me",
            Some(tok(&t)),
            json!({ "confirm": confirm }),
        )
    };
    assert_eq!(call(&c.app, del("nope")).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(
        me(&c, &t).await.0,
        StatusCode::OK,
        "wrong confirmation changes nothing"
    );
    assert_eq!(call(&c.app, del("delete")).await.0, StatusCode::NO_CONTENT);

    assert_eq!(me(&c, &t).await.0, StatusCode::UNAUTHORIZED);
    let id = t["account_id"].as_str().unwrap();
    for table in ["saves", "seeds", "sessions", "identities"] {
        let n: i64 = sqlx::query_scalar(&format!(
            "SELECT count(*) FROM {table} WHERE account_id = $1::uuid"
        ))
        .bind(id)
        .fetch_one(&c.pool)
        .await
        .unwrap();
        assert_eq!(n, 0, "{table} should be empty");
    }
    // The same provider identity starts a brand-new account.
    let t2 = login(&c, &sub).await;
    assert_ne!(t2["account_id"], t["account_id"]);
}

#[tokio::test]
async fn providers_endpoint_lists_only_configured() {
    let Some(c) = ctx().await else { return };
    let (st, _, b) = call(
        &c.app,
        Request::get("/v1/auth/providers")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let v: Value = serde_json::from_slice(&b).unwrap();
    assert_eq!(v["providers"], json!(["fake", "fake2"]));
}

#[tokio::test]
async fn old_style_unauthenticated_account_creation_is_gone() {
    let Some(c) = ctx().await else { return };
    let (st, ..) = call(
        &c.app,
        Request::post("/v1/accounts").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}
