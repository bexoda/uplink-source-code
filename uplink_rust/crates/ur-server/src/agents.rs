//! Agents: the login screen's roster for a signed-in player. An agent is a name bound to a
//! server-issued seed; its verified progress is the matching row in `runs`.

use axum::extract::{Path, State};
use axum::Json;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use ur_save::slot;

use crate::{Account, ApiError, AppState};

/// Agents that were created but never played still hold a seed; keep the roster bounded.
pub const MAX_AGENTS: i64 = 20;

#[derive(Deserialize)]
pub struct NewAgent {
    name: String,
}

#[derive(Serialize, Debug)]
pub struct AgentRow {
    pub name: String,
    /// Decimal string: u64 seeds don't fit a JS number.
    pub seed: String,
    /// 0 until the first segment is verified.
    pub verified_tick: i64,
    pub credits: Option<i64>,
    pub rating: Option<i32>,
    pub game_over: bool,
}

/// Creates an agent with a fresh seed. The name follows save-slot rules and is unique per account.
pub async fn create(
    State(s): State<AppState>,
    Account(acct): Account,
    Json(r): Json<NewAgent>,
) -> Result<Json<AgentRow>, ApiError> {
    if !slot::is_valid(&r.name) {
        return Err(ApiError::BadRequest(
            "agent names are 1-32 of a-z, 0-9, _ and -",
        ));
    }
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM seeds WHERE account_id = $1 AND agent_name IS NOT NULL",
    )
    .bind(acct)
    .fetch_one(&s.db)
    .await?;
    if n >= MAX_AGENTS {
        return Err(ApiError::Rejected("too many agents".into()));
    }
    let seed = OsRng.next_u64();
    let inserted =
        sqlx::query("INSERT INTO seeds (account_id, seed, agent_name) VALUES ($1, $2, $3)")
            .bind(acct)
            .bind(seed as i64)
            .bind(&r.name)
            .execute(&s.db)
            .await;
    match inserted {
        Ok(q) if q.rows_affected() == 1 => Ok(Json(AgentRow {
            name: r.name,
            seed: seed.to_string(),
            verified_tick: 0,
            credits: None,
            rating: None,
            game_over: false,
        })),
        Ok(_) => Err(ApiError::Internal),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            Err(ApiError::Taken("you already have an agent with that name"))
        }
        Err(e) => Err(e.into()),
    }
}

/// name, seed, verified_tick, credits, rating, game_over
type AgentSqlRow = (
    String,
    i64,
    Option<i64>,
    Option<i64>,
    Option<i32>,
    Option<bool>,
);

pub async fn list(
    State(s): State<AppState>,
    Account(acct): Account,
) -> Result<Json<Vec<AgentRow>>, ApiError> {
    let rows: Vec<AgentSqlRow> = sqlx::query_as(
        "SELECT s.agent_name, s.seed, r.verified_tick, r.credits, r.rating, r.game_over \
             FROM seeds s LEFT JOIN runs r ON r.account_id = s.account_id AND r.seed = s.seed \
             WHERE s.account_id = $1 AND s.agent_name IS NOT NULL ORDER BY s.agent_name",
    )
    .bind(acct)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(name, seed, vt, credits, rating, over)| AgentRow {
                name,
                seed: (seed as u64).to_string(),
                verified_tick: vt.unwrap_or(0),
                credits,
                rating,
                game_over: over.unwrap_or(false),
            })
            .collect(),
    ))
}

/// Retires an agent: its seed and verified run are deleted.
pub async fn retire(
    State(s): State<AppState>,
    Account(acct): Account,
    Path(name): Path<String>,
) -> Result<axum::http::StatusCode, ApiError> {
    let n = sqlx::query("DELETE FROM seeds WHERE account_id = $1 AND agent_name = $2")
        .bind(acct)
        .bind(&name)
        .execute(&s.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}
