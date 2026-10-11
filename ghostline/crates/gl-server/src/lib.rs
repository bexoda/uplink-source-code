//! Save-sync API. Stores opaque, client-encrypted blobs with compare-and-swap revisions.

use axum::extract::{DefaultBodyLimit, FromRequestParts, Path, State};
use axum::http::{header, request::Parts, HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use gl_save::slot;
use rand_core::{OsRng, RngCore};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tower_http::cors::{AllowOrigin, CorsLayer};
use uuid::Uuid;

pub const MAX_BLOB: usize = 2 * 1024 * 1024;
pub const MAX_SLOTS: i64 = 8;
const IF_REVISION: &str = "x-if-revision";
const REVISION: &str = "x-revision";

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    /// If set, `POST /v1/accounts` requires this in `x-registration-key`.
    pub registration_key: Option<String>,
}

pub async fn migrate(db: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(db).await
}

pub fn router(state: AppState, allowed_origin: Option<String>) -> Router {
    let cors = match allowed_origin.and_then(|o| HeaderValue::from_str(&o).ok()) {
        Some(o) => CorsLayer::new()
            .allow_origin(AllowOrigin::exact(o))
            .allow_methods([Method::GET, Method::PUT, Method::POST])
            .allow_headers([
                header::AUTHORIZATION,
                header::CONTENT_TYPE,
                HeaderName::from_static(IF_REVISION),
                HeaderName::from_static("x-registration-key"),
            ])
            .expose_headers([HeaderName::from_static(REVISION)]),
        // No origin configured: same-origin only.
        None => CorsLayer::new(),
    };
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/v1/accounts", post(create_account))
        .route("/v1/saves", get(list_saves))
        .route("/v1/saves/{slot}", get(get_save).put(put_save))
        .layer(DefaultBodyLimit::max(MAX_BLOB))
        .layer(cors)
        .with_state(state)
}

#[derive(Debug)]
pub enum ApiError {
    Unauthorized,
    Forbidden,
    BadRequest(&'static str),
    NotFound,
    Limit,
    Conflict { current: i64 },
    Db(sqlx::Error),
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        ApiError::Db(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized").into_response(),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "forbidden").into_response(),
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m).into_response(),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not found").into_response(),
            ApiError::Limit => {
                (StatusCode::UNPROCESSABLE_ENTITY, "slot limit reached").into_response()
            }
            ApiError::Conflict { current } => {
                let mut h = HeaderMap::new();
                h.insert(REVISION, HeaderValue::from(current as u64));
                (StatusCode::CONFLICT, h, "revision conflict").into_response()
            }
            ApiError::Db(e) => {
                tracing::error!("db error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
            }
        }
    }
}

/// Authenticated account, resolved from `Authorization: Bearer <token>`.
pub struct Account(pub Uuid);

impl FromRequestParts<AppState> for Account {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(ApiError::Unauthorized)?;
        let hash = Sha256::digest(token.as_bytes()).to_vec();
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM accounts WHERE token_hash = $1")
            .bind(hash)
            .fetch_optional(&state.db)
            .await?
            .map(Account)
            .ok_or(ApiError::Unauthorized)
    }
}

#[derive(Serialize)]
struct NewAccount {
    token: String,
}

async fn create_account(
    State(s): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<NewAccount>, ApiError> {
    if let Some(want) = &s.registration_key {
        let ok = headers
            .get("x-registration-key")
            .and_then(|v| v.to_str().ok())
            == Some(want.as_str());
        if !ok {
            return Err(ApiError::Forbidden);
        }
    }
    let mut raw = [0u8; 32];
    OsRng.fill_bytes(&mut raw);
    let token: String = raw.iter().map(|b| format!("{b:02x}")).collect();
    let hash = Sha256::digest(token.as_bytes()).to_vec();
    sqlx::query("INSERT INTO accounts (token_hash) VALUES ($1)")
        .bind(hash)
        .execute(&s.db)
        .await?;
    // Shown once; only the hash is stored.
    Ok(Json(NewAccount { token }))
}

#[derive(Serialize, sqlx::FromRow)]
struct SlotRow {
    slot: String,
    revision: i64,
    size: i64,
    updated_at: i64,
}

async fn list_saves(
    State(s): State<AppState>,
    Account(acct): Account,
) -> Result<Json<Vec<SlotRow>>, ApiError> {
    let rows = sqlx::query_as::<_, SlotRow>(
        "SELECT slot, revision, octet_length(blob)::bigint AS size, \
         extract(epoch FROM updated_at)::bigint AS updated_at \
         FROM saves WHERE account_id = $1 ORDER BY slot",
    )
    .bind(acct)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

async fn get_save(
    State(s): State<AppState>,
    Account(acct): Account,
    Path(slot_name): Path<String>,
) -> Result<Response, ApiError> {
    if !slot::is_valid(&slot_name) {
        return Err(ApiError::BadRequest("invalid slot"));
    }
    let (rev, blob): (i64, Vec<u8>) =
        sqlx::query_as("SELECT revision, blob FROM saves WHERE account_id = $1 AND slot = $2")
            .bind(acct)
            .bind(&slot_name)
            .fetch_optional(&s.db)
            .await?
            .ok_or(ApiError::NotFound)?;
    let mut h = HeaderMap::new();
    h.insert(REVISION, HeaderValue::from(rev as u64));
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    Ok((h, blob).into_response())
}

async fn put_save(
    State(s): State<AppState>,
    Account(acct): Account,
    Path(slot_name): Path<String>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Response, ApiError> {
    if !slot::is_valid(&slot_name) {
        return Err(ApiError::BadRequest("invalid slot"));
    }
    // Cheap sanity check; the server can't decrypt, but it can refuse junk.
    if gl_save::peek(&body).is_err() {
        return Err(ApiError::BadRequest("not a ghostline save"));
    }
    let expect: i64 = headers
        .get(IF_REVISION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .filter(|n| *n >= 0)
        .ok_or(ApiError::BadRequest("x-if-revision required"))?;

    let new_rev: Option<i64> = if expect == 0 {
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM saves WHERE account_id = $1")
            .bind(acct)
            .fetch_one(&s.db)
            .await?;
        if n >= MAX_SLOTS {
            return Err(ApiError::Limit);
        }
        sqlx::query_scalar(
            "INSERT INTO saves (account_id, slot, revision, blob) VALUES ($1, $2, 1, $3) \
             ON CONFLICT DO NOTHING RETURNING revision",
        )
        .bind(acct)
        .bind(&slot_name)
        .bind(body.as_ref())
        .fetch_optional(&s.db)
        .await?
    } else {
        sqlx::query_scalar(
            "UPDATE saves SET revision = revision + 1, blob = $4, updated_at = now() \
             WHERE account_id = $1 AND slot = $2 AND revision = $3 RETURNING revision",
        )
        .bind(acct)
        .bind(&slot_name)
        .bind(expect)
        .bind(body.as_ref())
        .fetch_optional(&s.db)
        .await?
    };

    match new_rev {
        Some(rev) => {
            let mut h = HeaderMap::new();
            h.insert(REVISION, HeaderValue::from(rev as u64));
            Ok((StatusCode::OK, h).into_response())
        }
        None => {
            let current: i64 = sqlx::query_scalar(
                "SELECT coalesce(max(revision), 0) FROM saves WHERE account_id = $1 AND slot = $2",
            )
            .bind(acct)
            .bind(&slot_name)
            .fetch_one(&s.db)
            .await?;
            Err(ApiError::Conflict { current })
        }
    }
}
