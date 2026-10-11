#![allow(dead_code)]
//! Runs against a real Postgres. Set TEST_DATABASE_URL, e.g.
//! postgres://gl:gl@localhost/gl_test ; skipped (loudly) when unset.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use gl_save::{seal, Key, Unlock};
use gl_server::{migrate, router, AppState};
use http_body_util::BodyExt;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

pub async fn app() -> Option<axum::Router> {
    let Ok(url) = std::env::var("TEST_DATABASE_URL") else {
        eprintln!("SKIPPED: TEST_DATABASE_URL not set");
        return None;
    };
    let db = PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .unwrap();
    migrate(&db).await.unwrap();
    Some(router(
        AppState {
            db,
            registration_key: Some("sekrit".into()),
            server_key: std::sync::Arc::new(Key::from_bytes([7u8; 32])),
        },
        None,
    ))
}

pub async fn call(
    app: &axum::Router,
    req: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let r = app.clone().oneshot(req).await.unwrap();
    let (p, b) = r.into_parts();
    (
        p.status,
        p.headers,
        b.collect().await.unwrap().to_bytes().to_vec(),
    )
}

pub fn rev(h: &axum::http::HeaderMap) -> u64 {
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

pub async fn register(app: &axum::Router) -> String {
    let req = Request::post("/v1/accounts")
        .header("x-registration-key", "sekrit")
        .body(Body::empty())
        .unwrap();
    let (st, _, body) = call(app, req).await;
    assert_eq!(st, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    v["token"].as_str().unwrap().to_string()
}

pub fn blob(seed: u64) -> Vec<u8> {
    seal(&gl_sim::World::new(seed), Unlock::Key(&Key::generate())).unwrap()
}
