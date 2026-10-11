mod common;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use serde_json::json;
use ur_save::encode_segment;
use ur_sim::replay::Run;
use ur_sim::*;

fn del(token: &str, path: &str) -> Request<Body> {
    Request::delete(path)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn agents_are_named_seeds_with_verified_progress() {
    let Some(app) = app().await else { return };
    let t = register(&app).await;

    let (st, _, body) = call(
        &app,
        json_req("POST", "/v1/agents", Some(&t), json!({"name": "neo"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let neo: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let seed: u64 = neo["seed"].as_str().unwrap().parse().unwrap();

    // Same name again: refused. Bad names: refused.
    let (st, _, _) = call(
        &app,
        json_req("POST", "/v1/agents", Some(&t), json!({"name": "neo"})),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);
    let (st, _, _) = call(
        &app,
        json_req(
            "POST",
            "/v1/agents",
            Some(&t),
            json!({"name": "Neo Anderson"}),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    // Named seeds don't use up the anonymous seed allowance.
    let (st, _, _) = call(&app, json_req("POST", "/v1/seeds?n=5", Some(&t), json!({}))).await;
    assert_eq!(st, StatusCode::OK);

    // Play the agent and upload: the roster reports the verified progress.
    let mut run = Run::new(seed);
    let mut ev = vec![];
    run.apply(Command::AcceptMission(1), &mut ev).unwrap();
    for _ in 0..50 {
        run.tick(&mut ev);
    }
    let req = Request::put(format!("/v1/runs/{seed}/segment"))
        .header("authorization", format!("Bearer {t}"))
        .body(Body::from(encode_segment(&run.segment()).unwrap()))
        .unwrap();
    let (st, _, body) = call(&app, req).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&body));

    let (st, _, body) = call(&app, get(&t, "/v1/agents")).await;
    assert_eq!(st, StatusCode::OK);
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["name"], "neo");
    assert_eq!(list[0]["seed"], seed.to_string());
    assert_eq!(list[0]["verified_tick"], 50);

    // Another account never sees it.
    let other = register(&app).await;
    let (_, _, body) = call(&app, get(&other, "/v1/agents")).await;
    assert_eq!(body, b"[]");
    let (st, _, _) = call(&app, del(&other, "/v1/agents/neo")).await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    // Retiring deletes the agent and its verified run.
    let (st, _, _) = call(&app, del(&t, "/v1/agents/neo")).await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (_, _, body) = call(&app, get(&t, "/v1/agents")).await;
    assert_eq!(body, b"[]");
    let (st, _, _) = call(&app, get(&t, &format!("/v1/runs/{seed}/state"))).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn agents_need_a_session() {
    let Some(app) = app().await else { return };
    let (st, _, _) = call(
        &app,
        json_req("POST", "/v1/agents", None, json!({"name": "x"})),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}
