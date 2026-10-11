//! Verified runs. The server never accepts client state: it accepts command logs and
//! replays them on its own copy of the world. Anything the sim doesn't allow is rejected.

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::Json;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use ur_save::{decode_segment, digest, encode_state, open, seal, Unlock};
use ur_sim::replay::{replay, Limits, SIM_VERSION};
use ur_sim::tuning::TICKS_PER_SECOND;
use ur_sim::World;

use crate::{Account, ApiError, AppState};

/// Grace on top of real elapsed time, for clock jitter and the tick in flight.
const SLACK_TICKS: i64 = 600;
const MAX_UNUSED_SEEDS: i64 = 10;

fn parse_seed(s: &str) -> Result<u64, ApiError> {
    s.parse::<u64>()
        .map_err(|_| ApiError::BadRequest("invalid seed"))
}

#[derive(Deserialize)]
pub struct SeedQuery {
    n: Option<u32>,
}

#[derive(Serialize)]
pub struct SeedList {
    /// Decimal strings: u64 seeds don't fit a JS number.
    seeds: Vec<String>,
}

/// Hand out seeds to start runs with. A client can fetch a few ahead and start offline.
pub async fn issue_seeds(
    State(s): State<AppState>,
    Account(acct): Account,
    Query(q): Query<SeedQuery>,
) -> Result<Json<SeedList>, ApiError> {
    let n = q.n.unwrap_or(1).clamp(1, 5) as i64;
    let unused: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM seeds WHERE account_id = $1 AND NOT used AND agent_name IS NULL",
    )
    .bind(acct)
    .fetch_one(&s.db)
    .await?;
    if unused + n > MAX_UNUSED_SEEDS {
        return Err(ApiError::Rejected("too many unused seeds".into()));
    }
    let mut seeds = Vec::new();
    for _ in 0..n {
        let seed = OsRng.next_u64();
        sqlx::query("INSERT INTO seeds (account_id, seed) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(acct)
            .bind(seed as i64)
            .execute(&s.db)
            .await?;
        seeds.push(seed.to_string());
    }
    Ok(Json(SeedList { seeds }))
}

#[derive(Serialize)]
pub struct SyncResult {
    verified_tick: u64,
    revision: i64,
    credits: i64,
    rating: u32,
    heat: u8,
    game_over: bool,
    /// Hex SHA-256 of the server's resulting world.
    digest: String,
}

pub async fn sync_segment(
    State(s): State<AppState>,
    Account(acct): Account,
    Path(seed_s): Path<String>,
    body: axum::body::Bytes,
) -> Result<Json<SyncResult>, ApiError> {
    let seed = parse_seed(&seed_s)?;
    let seg = decode_segment(&body).map_err(|_| ApiError::BadRequest("malformed segment"))?;
    if seg.sim_version != SIM_VERSION {
        return Err(ApiError::UpgradeRequired);
    }

    let mut tx = s.db.begin().await?;
    // Locking the seed row serialises every sync of this run, including the very first one.
    let elapsed: i64 = sqlx::query_scalar(
        "SELECT floor(extract(epoch FROM now() - issued_at))::bigint FROM seeds \
         WHERE account_id = $1 AND seed = $2 FOR UPDATE",
    )
    .bind(acct)
    .bind(seed as i64)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::NotFound)?;

    let row: Option<(i64, i64, Vec<u8>, bool)> =
        sqlx::query_as("SELECT verified_tick, revision, state, game_over FROM runs WHERE account_id = $1 AND seed = $2")
            .bind(acct)
            .bind(seed as i64)
            .fetch_optional(&mut *tx)
            .await?;

    let (mut world, verified, revision) = match row {
        None => (World::new(seed), 0i64, 0i64),
        Some((_, _, _, true)) => return Err(ApiError::Rejected("run is over".into())),
        Some((vt, rev, state, false)) => {
            let w = open(&state, Unlock::Key(&s.server_key)).map_err(|e| {
                tracing::error!("cannot unseal stored run: {e}");
                ApiError::Internal
            })?;
            (w, vt, rev)
        }
    };
    if seg.start_tick != verified as u64 {
        return Err(ApiError::RunConflict {
            verified_tick: verified,
        });
    }
    // You cannot have played more game time than real time has passed since the seed was issued.
    let budget = elapsed
        .max(0)
        .saturating_mul(TICKS_PER_SECOND as i64)
        .saturating_add(SLACK_TICKS);
    if seg.end_tick > budget as u64 {
        return Err(ApiError::Rejected(
            "claims more game time than has elapsed".into(),
        ));
    }

    // Untrusted input: replay off the async runtime, and treat a panic as a rejection.
    let outcome = tokio::task::spawn_blocking(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let r = replay(&mut world, &seg, Limits::default());
            (r, world)
        }))
    })
    .await
    .map_err(|_| ApiError::Internal)?;
    let world = match outcome {
        Err(_) => return Err(ApiError::Rejected("malformed log".into())),
        Ok((Err(e), _)) => return Err(ApiError::Rejected(format!("log rejected: {e:?}"))),
        Ok((Ok(()), w)) => w,
    };

    let sealed = seal(&world, Unlock::Key(&s.server_key)).map_err(|_| ApiError::Internal)?;
    let dg = digest(&world).map_err(|_| ApiError::Internal)?;
    let new_rev = revision + 1;
    sqlx::query(
        "INSERT INTO runs (account_id, seed, sim_version, verified_tick, revision, state, credits, rating, heat, game_over) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
         ON CONFLICT (account_id, seed) DO UPDATE SET \
           verified_tick = EXCLUDED.verified_tick, revision = EXCLUDED.revision, state = EXCLUDED.state, \
           credits = EXCLUDED.credits, rating = EXCLUDED.rating, heat = EXCLUDED.heat, \
           game_over = EXCLUDED.game_over, updated_at = now()",
    )
    .bind(acct)
    .bind(seed as i64)
    .bind(SIM_VERSION as i32)
    .bind(world.tick_no as i64)
    .bind(new_rev)
    .bind(&sealed)
    .bind(world.player.credits)
    .bind(world.player.rating as i32)
    .bind(world.player.heat as i16)
    .bind(world.game_over)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE seeds SET used = true WHERE account_id = $1 AND seed = $2")
        .bind(acct)
        .bind(seed as i64)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(Json(SyncResult {
        verified_tick: world.tick_no,
        revision: new_rev,
        credits: world.player.credits,
        rating: world.player.rating,
        heat: world.player.heat,
        game_over: world.game_over,
        digest: dg.iter().map(|b| format!("{b:02x}")).collect(),
    }))
}

/// The server's verified world, for a new device to resume from.
pub async fn get_state(
    State(s): State<AppState>,
    Account(acct): Account,
    Path(seed_s): Path<String>,
) -> Result<Response, ApiError> {
    let seed = parse_seed(&seed_s)?;
    let (vt, state): (i64, Vec<u8>) =
        sqlx::query_as("SELECT verified_tick, state FROM runs WHERE account_id = $1 AND seed = $2")
            .bind(acct)
            .bind(seed as i64)
            .fetch_optional(&s.db)
            .await?
            .ok_or(ApiError::NotFound)?;
    let world = open(&state, Unlock::Key(&s.server_key)).map_err(|_| ApiError::Internal)?;
    let bytes = encode_state(&world).map_err(|_| ApiError::Internal)?;
    let mut h = HeaderMap::new();
    h.insert("x-verified-tick", HeaderValue::from(vt as u64));
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    Ok((h, bytes).into_response())
}

#[derive(Serialize)]
pub struct RunRow {
    seed: String,
    verified_tick: i64,
    revision: i64,
    credits: i64,
    rating: i32,
    heat: i16,
    game_over: bool,
}

pub async fn list_runs(
    State(s): State<AppState>,
    Account(acct): Account,
) -> Result<Json<Vec<RunRow>>, ApiError> {
    let rows: Vec<(i64, i64, i64, i64, i32, i16, bool)> = sqlx::query_as(
        "SELECT seed, verified_tick, revision, credits, rating, heat, game_over FROM runs \
         WHERE account_id = $1 ORDER BY updated_at DESC",
    )
    .bind(acct)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(
                |(seed, verified_tick, revision, credits, rating, heat, game_over)| RunRow {
                    seed: (seed as u64).to_string(),
                    verified_tick,
                    revision,
                    credits,
                    rating,
                    heat,
                    game_over,
                },
            )
            .collect(),
    ))
}
