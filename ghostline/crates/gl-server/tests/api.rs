mod common;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;

#[tokio::test]
async fn auth_required_and_isolated_between_accounts() {
    let Some(app) = app().await else { return };
    let (a, b) = (register(&app).await, register(&app).await);
    let (st, ..) = call(&app, Request::get("/v1/saves").body(Body::empty()).unwrap()).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (st, ..) = call(&app, get("deadbeef", "/v1/saves")).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    let (st, ..) = call(&app, put(&a, "main", 0, blob(1))).await;
    assert_eq!(st, StatusCode::OK);
    let (st, ..) = call(&app, get(&b, "/v1/saves/main")).await;
    assert_eq!(st, StatusCode::NOT_FOUND, "account B must not see A's save");
}

#[tokio::test]
async fn push_pull_and_compare_and_swap() {
    let Some(app) = app().await else { return };
    let t = register(&app).await;
    let (b1, b2, b3) = (blob(1), blob(2), blob(3));

    let (st, h, _) = call(&app, put(&t, "main", 0, b1.clone())).await;
    assert_eq!((st, rev(&h)), (StatusCode::OK, 1));

    // Creating again with expect=0 must conflict, not overwrite.
    let (st, h, _) = call(&app, put(&t, "main", 0, b2.clone())).await;
    assert_eq!((st, rev(&h)), (StatusCode::CONFLICT, 1));

    let (st, h, _) = call(&app, put(&t, "main", 1, b2.clone())).await;
    assert_eq!((st, rev(&h)), (StatusCode::OK, 2));

    // A second device still on revision 1 loses.
    let (st, h, _) = call(&app, put(&t, "main", 1, b3)).await;
    assert_eq!((st, rev(&h)), (StatusCode::CONFLICT, 2));

    let (st, h, body) = call(&app, get(&t, "/v1/saves/main")).await;
    assert_eq!((st, rev(&h)), (StatusCode::OK, 2));
    assert_eq!(body, b2, "blob must round-trip byte-for-byte");

    let (_, _, body) = call(&app, get(&t, "/v1/saves")).await;
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v[0]["slot"], "main");
    assert_eq!(v[0]["revision"], 2);
    assert_eq!(v[0]["size"], b2.len());
}

#[tokio::test]
async fn concurrent_writers_exactly_one_wins() {
    let Some(app) = app().await else { return };
    let t = register(&app).await;
    call(&app, put(&t, "race", 0, blob(1))).await;
    let mut set = tokio::task::JoinSet::new();
    for i in 0..8u64 {
        let (app, t) = (app.clone(), t.clone());
        set.spawn(async move { call(&app, put(&t, "race", 1, blob(10 + i))).await.0 });
    }
    let mut ok = 0;
    while let Some(r) = set.join_next().await {
        if r.unwrap() == StatusCode::OK {
            ok += 1;
        }
    }
    assert_eq!(ok, 1);
}

#[tokio::test]
async fn rejects_bad_input() {
    let Some(app) = app().await else { return };
    let t = register(&app).await;
    let (st, ..) = call(
        &app,
        put(&t, "main", 0, b"not a save at all, just text".to_vec()),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, ..) = call(&app, put(&t, "BAD_SLOT", 0, blob(1))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let req = Request::put("/v1/saves/main")
        .header("authorization", format!("Bearer {t}"))
        .body(Body::from(blob(1)))
        .unwrap();
    let (st, ..) = call(&app, req).await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "missing x-if-revision");
    let (st, ..) = call(&app, put(&t, "big", 0, vec![0u8; 3 * 1024 * 1024])).await;
    assert_eq!(st, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn slot_limit() {
    let Some(app) = app().await else { return };
    let t = register(&app).await;
    for i in 0..8 {
        let (st, ..) = call(&app, put(&t, &format!("s{i}"), 0, blob(1))).await;
        assert_eq!(st, StatusCode::OK);
    }
    let (st, ..) = call(&app, put(&t, "s8", 0, blob(1))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
}
