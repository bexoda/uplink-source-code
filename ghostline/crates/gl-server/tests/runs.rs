mod common;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use gl_save::{decode_state, digest, encode_segment};
use gl_sim::replay::*;
use gl_sim::*;

async fn new_seed(app: &axum::Router, token: &str) -> u64 {
    let req = Request::post("/v1/seeds")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let (st, _, body) = call(app, req).await;
    assert_eq!(st, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    v["seeds"][0].as_str().unwrap().parse().unwrap()
}

fn put_seg(token: &str, seed: u64, seg: &Segment) -> Request<Body> {
    put_seg_bytes(token, seed, encode_segment(seg).unwrap())
}

fn put_seg_bytes(token: &str, seed: u64, body: Vec<u8>) -> Request<Body> {
    Request::put(format!("/v1/runs/{seed}/segment"))
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap()
}

/// A short, always-legal session on any seed.
fn play(run: &mut Run, ticks: u32) {
    let mut ev = vec![];
    let w = &run.world;
    let relays: Vec<_> = w
        .servers
        .iter()
        .filter(|(_, s)| s.kind == ServerKind::Relay)
        .map(|(i, _)| i)
        .collect();
    let corp = w
        .servers
        .iter()
        .find(|(_, s)| s.kind == ServerKind::Corp)
        .map(|(i, _)| i)
        .unwrap();
    run.apply(Command::AcceptMission(1), &mut ev).unwrap();
    run.apply(Command::BuyTool(Tool::Decrypter), &mut ev)
        .unwrap();
    run.apply(
        Command::Connect {
            chain: relays,
            target: corp,
        },
        &mut ev,
    )
    .unwrap();
    for _ in 0..ticks {
        run.tick(&mut ev);
    }
}

#[tokio::test]
async fn cheater_edits_credits_but_server_computes_the_honest_number() {
    let Some(app) = app().await else { return };
    let t = register(&app).await;
    let seed = new_seed(&app, &t).await;

    let mut run = Run::new(seed);
    play(&mut run, 300);
    let honest = run.world.player.credits;
    let honest_digest = digest(&run.world).unwrap();

    // What Cheat Engine does: edit the number in memory. The command log is untouched.
    run.world.player.credits = 999_999_999;

    let (st, _, body) = call(&app, put_seg(&t, seed, &run.segment())).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["credits"], honest);
    assert_ne!(v["credits"], 999_999_999);
    let hex: String = honest_digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        v["digest"], hex,
        "server world must equal the honestly-played world"
    );

    // A second device can resume from the verified state.
    let (st, h, body) = call(&app, get(&t, &format!("/v1/runs/{seed}/state"))).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(
        h["x-verified-tick"].to_str().unwrap(),
        run.world.tick_no.to_string()
    );
    assert_eq!(
        digest(&decode_state(&body).unwrap()).unwrap(),
        honest_digest
    );
}

#[tokio::test]
async fn segments_chain_and_replays_conflict() {
    let Some(app) = app().await else { return };
    let t = register(&app).await;
    let seed = new_seed(&app, &t).await;
    let mut run = Run::new(seed);
    play(&mut run, 200);
    let seg1 = run.segment();
    assert_eq!(call(&app, put_seg(&t, seed, &seg1)).await.0, StatusCode::OK);

    // Re-sending the same segment must not apply twice.
    let (st, h, _) = call(&app, put_seg(&t, seed, &seg1)).await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert_eq!(
        h["x-verified-tick"].to_str().unwrap(),
        seg1.end_tick.to_string()
    );

    run.acknowledge(&seg1);
    run.apply(Command::Disconnect, &mut vec![]).unwrap();
    run.tick(&mut vec![]);
    let (st, _, body) = call(&app, put_seg(&t, seed, &run.segment())).await;
    assert_eq!(st, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["revision"], 2);

    // Skipping ahead (a gap in the chain) is refused.
    let mut gap = run.segment();
    gap.start_tick += 5;
    assert_eq!(
        call(&app, put_seg(&t, seed, &gap)).await.0,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn forged_logs_are_rejected_and_do_not_change_state() {
    let Some(app) = app().await else { return };
    let t = register(&app).await;
    let seed = new_seed(&app, &t).await;
    let mut run = Run::new(seed);
    play(&mut run, 50);

    // Buying a second Decrypter with money you don't have.
    let mut forged = run.segment();
    forged.entries.push(CommandEntry {
        tick: forged.end_tick,
        cmd: Command::BuyTool(Tool::Decrypter),
    });
    forged.entries.push(CommandEntry {
        tick: forged.end_tick,
        cmd: Command::BuyTool(Tool::Decrypter),
    });
    forged.entries.push(CommandEntry {
        tick: forged.end_tick,
        cmd: Command::BuyTool(Tool::Decrypter),
    });
    let (st, ..) = call(&app, put_seg(&t, seed, &forged)).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);

    // Nothing was stored, so the honest log still works afterwards.
    let (st, ..) = call(&app, get(&t, &format!("/v1/runs/{seed}/state"))).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    assert_eq!(
        call(&app, put_seg(&t, seed, &run.segment())).await.0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn cannot_claim_more_game_time_than_real_time() {
    let Some(app) = app().await else { return };
    let t = register(&app).await;
    let seed = new_seed(&app, &t).await;
    let mut run = Run::new(seed);
    play(&mut run, 5_000); // ~8 minutes of game time, seed issued milliseconds ago
    let (st, _, body) = call(&app, put_seg(&t, seed, &run.segment())).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(String::from_utf8_lossy(&body).contains("elapsed"));
}

#[tokio::test]
async fn seeds_must_be_server_issued_and_are_private_to_the_account() {
    let Some(app) = app().await else { return };
    let (a, b) = (register(&app).await, register(&app).await);
    let seed = new_seed(&app, &a).await;
    let mut run = Run::new(seed);
    play(&mut run, 10);

    // A seed the player picked themselves.
    let own = 42u64;
    let mut mine = Run::new(own);
    play(&mut mine, 10);
    assert_eq!(
        call(&app, put_seg(&a, own, &mine.segment())).await.0,
        StatusCode::NOT_FOUND
    );
    // Someone else's issued seed.
    assert_eq!(
        call(&app, put_seg(&b, seed, &run.segment())).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn bad_versions_and_garbage() {
    let Some(app) = app().await else { return };
    let t = register(&app).await;
    let seed = new_seed(&app, &t).await;
    let mut run = Run::new(seed);
    play(&mut run, 10);
    let mut old = run.segment();
    old.sim_version = SIM_VERSION + 1;
    assert_eq!(
        call(&app, put_seg(&t, seed, &old)).await.0,
        StatusCode::UPGRADE_REQUIRED
    );
    assert_eq!(
        call(&app, put_seg_bytes(&t, seed, b"garbage".to_vec()))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(&app, put_seg_bytes(&t, 1, vec![])).await.0,
        StatusCode::BAD_REQUEST
    );
    let req = Request::put("/v1/runs/notanumber/segment")
        .header("authorization", format!("Bearer {t}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(call(&app, req).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn concurrent_identical_syncs_apply_exactly_once() {
    let Some(app) = app().await else { return };
    let t = register(&app).await;
    let seed = new_seed(&app, &t).await;
    let mut run = Run::new(seed);
    play(&mut run, 100);
    let body = encode_segment(&run.segment()).unwrap();
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..6 {
        let (app, t, body) = (app.clone(), t.clone(), body.clone());
        set.spawn(async move { call(&app, put_seg_bytes(&t, seed, body)).await.0 });
    }
    let (mut ok, mut conflict) = (0, 0);
    while let Some(r) = set.join_next().await {
        match r.unwrap() {
            StatusCode::OK => ok += 1,
            StatusCode::CONFLICT => conflict += 1,
            other => panic!("unexpected {other}"),
        }
    }
    assert_eq!((ok, conflict), (1, 5));
}

#[tokio::test]
async fn unused_seed_cap() {
    let Some(app) = app().await else { return };
    let t = register(&app).await;
    for _ in 0..2 {
        let req = Request::post("/v1/seeds?n=5")
            .header("authorization", format!("Bearer {t}"))
            .body(Body::empty())
            .unwrap();
        assert_eq!(call(&app, req).await.0, StatusCode::OK);
    }
    let req = Request::post("/v1/seeds")
        .header("authorization", format!("Bearer {t}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(call(&app, req).await.0, StatusCode::UNPROCESSABLE_ENTITY);
}
