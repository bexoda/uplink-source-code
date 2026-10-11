//! OAuth login (backend-for-frontend), sessions and account management.
//!
//! The server runs the whole provider flow; clients never see provider tokens. A successful
//! login yields a one-time `login_code` that only the client holding the matching secret can
//! redeem for our own short-lived access token and rotating refresh token.
//!
//! Identity comes from the provider's userinfo endpoint, called with the access token we got
//! directly from the token endpoint over TLS. OIDC Core 3.1.3.7 allows this in place of ID-token
//! signature checks, and it keeps every provider (OIDC or plain OAuth2) on one code path.
//! Accounts are keyed on the provider's stable subject id, never on email.

use std::collections::BTreeMap;

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap};
use axum::response::Redirect;
use axum::Json;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use url::Url;
use uuid::Uuid;

use crate::{Account, ApiError, AppState};

pub const ACCESS_TTL_SECS: i64 = 15 * 60;
pub const REFRESH_TTL_SECS: i64 = 60 * 24 * 3600;
const RESERVED_HANDLES: [&str; 7] = [
    "admin",
    "root",
    "system",
    "moderator",
    "uplink_rust",
    "ghostline",
    "support",
];

#[derive(Clone, Debug)]
pub struct ProviderConfig {
    pub name: String,
    pub authorize_url: String,
    pub token_url: String,
    pub userinfo_url: String,
    pub scopes: Vec<String>,
    pub client_id: String,
    pub client_secret: String,
    /// JSON field in the userinfo response holding the stable user id.
    pub subject_field: &'static str,
    /// Only recorded when the provider also vouches that it is verified.
    pub email_field: Option<&'static str>,
    pub verified_field: Option<&'static str>,
}

impl ProviderConfig {
    pub fn google(client_id: String, client_secret: String) -> Self {
        ProviderConfig {
            name: "google".into(),
            authorize_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_url: "https://oauth2.googleapis.com/token".into(),
            userinfo_url: "https://openidconnect.googleapis.com/v1/userinfo".into(),
            scopes: vec!["openid".into(), "email".into()],
            client_id,
            client_secret,
            subject_field: "sub",
            email_field: Some("email"),
            verified_field: Some("email_verified"),
        }
    }

    /// GitHub is OAuth2 only; no scopes are requested, so we only learn the public numeric id.
    pub fn github(client_id: String, client_secret: String) -> Self {
        ProviderConfig {
            name: "github".into(),
            authorize_url: "https://github.com/login/oauth/authorize".into(),
            token_url: "https://github.com/login/oauth/access_token".into(),
            userinfo_url: "https://api.github.com/user".into(),
            scopes: vec![],
            client_id,
            client_secret,
            subject_field: "id",
            email_field: None,
            verified_field: None,
        }
    }

    pub fn discord(client_id: String, client_secret: String) -> Self {
        ProviderConfig {
            name: "discord".into(),
            authorize_url: "https://discord.com/oauth2/authorize".into(),
            token_url: "https://discord.com/api/oauth2/token".into(),
            userinfo_url: "https://discord.com/api/users/@me".into(),
            scopes: vec!["identify".into()],
            client_id,
            client_secret,
            subject_field: "id",
            email_field: None,
            verified_field: None,
        }
    }
}

/// Providers enabled by `UPLINK_RUST_OAUTH_<NAME>_CLIENT_ID` / `_CLIENT_SECRET`.
pub fn providers_from_env() -> BTreeMap<String, ProviderConfig> {
    let mut m = BTreeMap::new();
    for (name, make) in [
        (
            "GOOGLE",
            ProviderConfig::google as fn(String, String) -> ProviderConfig,
        ),
        ("GITHUB", ProviderConfig::github),
        ("DISCORD", ProviderConfig::discord),
    ] {
        let id = std::env::var(format!("UPLINK_RUST_OAUTH_{name}_CLIENT_ID"));
        let secret = std::env::var(format!("UPLINK_RUST_OAUTH_{name}_CLIENT_SECRET"));
        if let (Ok(id), Ok(secret)) = (id, secret) {
            let p = make(id, secret);
            m.insert(p.name.clone(), p);
        }
    }
    m
}

fn rand_token() -> String {
    let mut b = [0u8; 32];
    OsRng.fill_bytes(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn sha(s: &str) -> Vec<u8> {
    Sha256::digest(s.as_bytes()).to_vec()
}

fn s256(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// A SHA-256 challenge is always 43 base64url characters.
fn valid_challenge(c: &str) -> bool {
    c.len() == 43
        && c.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub(crate) fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

pub(crate) async fn account_for_token(
    db: &PgPool,
    token: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT account_id FROM sessions WHERE access_hash = $1 AND access_expires > now()",
    )
    .bind(sha(token))
    .fetch_optional(db)
    .await
}

fn with_params(base: &str, params: &[(&str, &str)]) -> String {
    let mut u = Url::parse(base).expect("return_to was validated");
    u.query_pairs_mut().extend_pairs(params);
    u.to_string()
}

impl AppState {
    /// Where we may send the user after login. Open redirects here would leak login codes,
    /// so this is an exact-origin allowlist, plus loopback for the desktop client.
    pub fn return_to_allowed(&self, return_to: &str) -> bool {
        let Ok(u) = Url::parse(return_to) else {
            return false;
        };
        if !u.username().is_empty() || u.password().is_some() || u.fragment().is_some() {
            return false;
        }
        match (u.scheme(), u.host_str()) {
            ("http", Some("127.0.0.1" | "localhost" | "[::1]")) => true,
            ("http" | "https", Some(_)) => self
                .web_origins
                .iter()
                .any(|o| *o == u.origin().ascii_serialization()),
            _ => false,
        }
    }

    fn redirect_uri(&self, provider: &str) -> String {
        format!(
            "{}/v1/auth/{provider}/callback",
            self.public_url.trim_end_matches('/')
        )
    }
}

// ---------- tokens ----------

#[derive(Serialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub account_id: Uuid,
    pub handle: Option<String>,
    pub needs_handle: bool,
}

async fn handle_of(db: &PgPool, account: Uuid) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT handle FROM accounts WHERE id = $1")
        .bind(account)
        .fetch_one(db)
        .await
}

pub async fn issue_session(db: &PgPool, account: Uuid) -> Result<Tokens, ApiError> {
    let (access, refresh) = (rand_token(), rand_token());
    sqlx::query(
        "INSERT INTO sessions (account_id, access_hash, access_expires, refresh_hash, refresh_expires) \
         VALUES ($1, $2, now() + ($4::bigint * interval '1 second'), $3, now() + ($5::bigint * interval '1 second'))",
    )
    .bind(account)
    .bind(sha(&access))
    .bind(sha(&refresh))
    .bind(ACCESS_TTL_SECS)
    .bind(REFRESH_TTL_SECS)
    .execute(db)
    .await?;
    let handle = handle_of(db, account).await?;
    Ok(Tokens {
        access_token: access,
        refresh_token: refresh,
        expires_in: ACCESS_TTL_SECS,
        account_id: account,
        needs_handle: handle.is_none(),
        handle,
    })
}

/// Creates an account with a session, bypassing OAuth. For tests and local tooling only.
#[doc(hidden)]
pub async fn create_account_with_session(db: &PgPool) -> Result<Tokens, ApiError> {
    let id: Uuid = sqlx::query_scalar("INSERT INTO accounts DEFAULT VALUES RETURNING id")
        .fetch_one(db)
        .await?;
    issue_session(db, id).await
}

// ---------- OAuth flow ----------

#[derive(Serialize)]
pub struct ProviderList {
    providers: Vec<String>,
}

pub async fn list_providers(State(s): State<AppState>) -> Json<ProviderList> {
    Json(ProviderList {
        providers: s.providers.keys().cloned().collect(),
    })
}

#[derive(Deserialize)]
pub struct StartQuery {
    return_to: String,
    client_challenge: String,
}

pub async fn start(
    State(s): State<AppState>,
    Path(provider): Path<String>,
    Query(q): Query<StartQuery>,
) -> Result<Redirect, ApiError> {
    let url = begin(&s, &provider, &q.return_to, &q.client_challenge, None).await?;
    Ok(Redirect::to(&url))
}

#[derive(Deserialize)]
pub struct LinkReq {
    return_to: String,
    client_challenge: String,
}

#[derive(Serialize)]
pub struct UrlResp {
    url: String,
}

/// Link another provider to the logged-in account. The intent is stored server-side against the
/// session's account, so a crafted URL can't attach someone else's login to a victim's account.
pub async fn start_link(
    State(s): State<AppState>,
    Account(acct): Account,
    Path(provider): Path<String>,
    Json(r): Json<LinkReq>,
) -> Result<Json<UrlResp>, ApiError> {
    let url = begin(&s, &provider, &r.return_to, &r.client_challenge, Some(acct)).await?;
    Ok(Json(UrlResp { url }))
}

async fn begin(
    s: &AppState,
    provider: &str,
    return_to: &str,
    challenge: &str,
    link: Option<Uuid>,
) -> Result<String, ApiError> {
    let p = s.providers.get(provider).ok_or(ApiError::NotFound)?;
    if !s.return_to_allowed(return_to) {
        return Err(ApiError::BadRequest("return_to not allowed"));
    }
    if !valid_challenge(challenge) {
        return Err(ApiError::BadRequest("invalid client_challenge"));
    }
    sqlx::query("DELETE FROM oauth_states WHERE created_at < now() - interval '10 minutes'")
        .execute(&s.db)
        .await?;
    let (state, verifier) = (rand_token(), rand_token());
    sqlx::query(
        "INSERT INTO oauth_states (state_hash, provider, pkce_verifier, return_to, client_challenge, link_account) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(sha(&state))
    .bind(provider)
    .bind(&verifier)
    .bind(return_to)
    .bind(challenge)
    .bind(link)
    .execute(&s.db)
    .await?;

    let mut u = Url::parse(&p.authorize_url).map_err(|_| ApiError::Internal)?;
    {
        let mut q = u.query_pairs_mut();
        q.append_pair("response_type", "code")
            .append_pair("client_id", &p.client_id)
            .append_pair("redirect_uri", &s.redirect_uri(provider))
            .append_pair("state", &state)
            .append_pair("code_challenge", &s256(&verifier))
            .append_pair("code_challenge_method", "S256");
        if !p.scopes.is_empty() {
            q.append_pair("scope", &p.scopes.join(" "));
        }
    }
    Ok(u.to_string())
}

struct Identity {
    subject: String,
    email: Option<String>,
}

async fn fetch_identity(
    s: &AppState,
    p: &ProviderConfig,
    code: &str,
    verifier: &str,
) -> Result<Identity, ()> {
    let redirect = s.redirect_uri(&p.name);
    let tok: serde_json::Value = s
        .http
        .post(&p.token_url)
        .header(header::ACCEPT, "application/json")
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &redirect),
            ("client_id", &p.client_id),
            ("client_secret", &p.client_secret),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .map_err(|_| ())?
        .error_for_status()
        .map_err(|_| ())?
        .json()
        .await
        .map_err(|_| ())?;
    let access = tok["access_token"].as_str().ok_or(())?;
    let info: serde_json::Value = s
        .http
        .get(&p.userinfo_url)
        .bearer_auth(access)
        .header(header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|_| ())?
        .error_for_status()
        .map_err(|_| ())?
        .json()
        .await
        .map_err(|_| ())?;
    let subject = match &info[p.subject_field] {
        serde_json::Value::String(v) if !v.is_empty() => v.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => return Err(()),
    };
    let verified = p
        .verified_field
        .map(|f| info[f].as_bool() == Some(true))
        .unwrap_or(false);
    let email = p
        .email_field
        .filter(|_| verified)
        .and_then(|f| info[f].as_str())
        .map(str::to_owned);
    Ok(Identity { subject, email })
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

pub async fn callback(
    State(s): State<AppState>,
    Path(provider): Path<String>,
    Query(q): Query<CallbackQuery>,
) -> Result<Redirect, ApiError> {
    let state = q.state.ok_or(ApiError::BadRequest("missing state"))?;
    // Single use: consuming the row is what makes a replayed callback fail.
    let row: Option<(String, String, String, String, Option<Uuid>)> = sqlx::query_as(
        "DELETE FROM oauth_states WHERE state_hash = $1 AND created_at > now() - interval '10 minutes' \
         RETURNING provider, pkce_verifier, return_to, client_challenge, link_account",
    )
    .bind(sha(&state))
    .fetch_optional(&s.db)
    .await?;
    let (prov, verifier, return_to, challenge, link) =
        row.ok_or(ApiError::BadRequest("invalid or expired state"))?;
    if prov != provider {
        return Err(ApiError::BadRequest("invalid or expired state"));
    }
    // From here the state is trusted, so return_to is safe to redirect to (validated at start).
    let fail = |why: &str| Ok(Redirect::to(&with_params(&return_to, &[("error", why)])));
    if q.error.is_some() {
        return fail("access_denied");
    }
    let Some(code) = q.code else {
        return fail("access_denied");
    };
    let p = s.providers.get(&provider).ok_or(ApiError::NotFound)?;
    let Ok(id) = fetch_identity(&s, p, &code, &verifier).await else {
        return fail("provider_failed");
    };

    match link {
        Some(acct) => match link_identity(&s.db, acct, &provider, &id).await? {
            true => Ok(Redirect::to(&with_params(
                &return_to,
                &[("linked", &provider)],
            ))),
            false => fail("identity_in_use"),
        },
        None => {
            let acct = login_or_create(&s.db, &provider, &id).await?;
            let login_code = rand_token();
            sqlx::query("INSERT INTO login_codes (code_hash, account_id, client_challenge) VALUES ($1, $2, $3)")
                .bind(sha(&login_code))
                .bind(acct)
                .bind(&challenge)
                .execute(&s.db)
                .await?;
            Ok(Redirect::to(&with_params(
                &return_to,
                &[("login_code", &login_code)],
            )))
        }
    }
}

async fn find_account(
    db: &PgPool,
    provider: &str,
    subject: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar("SELECT account_id FROM identities WHERE provider = $1 AND subject = $2")
        .bind(provider)
        .bind(subject)
        .fetch_optional(db)
        .await
}

async fn login_or_create(db: &PgPool, provider: &str, id: &Identity) -> Result<Uuid, ApiError> {
    if let Some(a) = find_account(db, provider, &id.subject).await? {
        return Ok(a);
    }
    let mut tx = db.begin().await?;
    let acct: Uuid = sqlx::query_scalar("INSERT INTO accounts DEFAULT VALUES RETURNING id")
        .fetch_one(&mut *tx)
        .await?;
    let inserted = sqlx::query("INSERT INTO identities (provider, subject, account_id, email) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING")
        .bind(provider)
        .bind(&id.subject)
        .bind(acct)
        .bind(&id.email)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if inserted == 1 {
        tx.commit().await?;
        return Ok(acct);
    }
    // Lost a race with a concurrent first login for the same identity: discard our account.
    tx.rollback().await?;
    find_account(db, provider, &id.subject)
        .await?
        .ok_or(ApiError::Internal)
}

/// Returns false when the identity already belongs to a different account, or the account
/// already has a different login for this provider. Accounts are never merged automatically.
async fn link_identity(
    db: &PgPool,
    acct: Uuid,
    provider: &str,
    id: &Identity,
) -> Result<bool, ApiError> {
    if let Some(owner) = find_account(db, provider, &id.subject).await? {
        return Ok(owner == acct);
    }
    let r = sqlx::query("INSERT INTO identities (provider, subject, account_id, email) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING")
        .bind(provider)
        .bind(&id.subject)
        .bind(acct)
        .bind(&id.email)
        .execute(db)
        .await?;
    Ok(r.rows_affected() == 1)
}

#[derive(Deserialize)]
pub struct ExchangeReq {
    login_code: String,
    client_verifier: String,
}

pub async fn exchange(
    State(s): State<AppState>,
    Json(r): Json<ExchangeReq>,
) -> Result<Json<Tokens>, ApiError> {
    // The code is burned whether or not the verifier matches, so it can't be guessed against.
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "DELETE FROM login_codes WHERE code_hash = $1 AND created_at > now() - interval '60 seconds' \
         RETURNING account_id, client_challenge",
    )
    .bind(sha(&r.login_code))
    .fetch_optional(&s.db)
    .await?;
    let (acct, challenge) = row.ok_or(ApiError::Unauthorized)?;
    if s256(&r.client_verifier) != challenge {
        return Err(ApiError::Unauthorized);
    }
    Ok(Json(issue_session(&s.db, acct).await?))
}

#[derive(Deserialize)]
pub struct RefreshReq {
    refresh_token: String,
}

pub async fn refresh(
    State(s): State<AppState>,
    Json(r): Json<RefreshReq>,
) -> Result<Json<Tokens>, ApiError> {
    let h = sha(&r.refresh_token);
    let mut tx = s.db.begin().await?;
    let row: Option<(Uuid, Uuid, bool)> = sqlx::query_as(
        "SELECT id, account_id, refresh_hash = $1 FROM sessions \
         WHERE (refresh_hash = $1 OR prev_refresh_hash = $1) AND refresh_expires > now() FOR UPDATE",
    )
    .bind(&h)
    .fetch_optional(&mut *tx)
    .await?;
    let (sid, acct, is_current) = row.ok_or(ApiError::Unauthorized)?;
    if !is_current {
        // An already-rotated token came back: either a thief or a stale copy. Kill the session.
        sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(sid)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Err(ApiError::Unauthorized);
    }
    let (access, refresh) = (rand_token(), rand_token());
    sqlx::query(
        "UPDATE sessions SET prev_refresh_hash = refresh_hash, refresh_hash = $2, access_hash = $3, \
         access_expires = now() + ($4::bigint * interval '1 second'), refresh_expires = now() + ($5::bigint * interval '1 second') \
         WHERE id = $1",
    )
    .bind(sid)
    .bind(sha(&refresh))
    .bind(sha(&access))
    .bind(ACCESS_TTL_SECS)
    .bind(REFRESH_TTL_SECS)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let handle = handle_of(&s.db, acct).await?;
    Ok(Json(Tokens {
        access_token: access,
        refresh_token: refresh,
        expires_in: ACCESS_TTL_SECS,
        account_id: acct,
        needs_handle: handle.is_none(),
        handle,
    }))
}

pub async fn logout(
    State(s): State<AppState>,
    headers: HeaderMap,
) -> Result<axum::http::StatusCode, ApiError> {
    let token = bearer(&headers).ok_or(ApiError::Unauthorized)?;
    sqlx::query("DELETE FROM sessions WHERE access_hash = $1")
        .bind(sha(token))
        .execute(&s.db)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ---------- account ----------

#[derive(Serialize, sqlx::FromRow)]
pub struct IdentityRow {
    provider: String,
    email: Option<String>,
}

#[derive(Serialize)]
pub struct Me {
    account_id: Uuid,
    handle: Option<String>,
    needs_handle: bool,
    identities: Vec<IdentityRow>,
}

pub async fn me(State(s): State<AppState>, Account(acct): Account) -> Result<Json<Me>, ApiError> {
    let handle = handle_of(&s.db, acct).await?;
    let identities = sqlx::query_as::<_, IdentityRow>(
        "SELECT provider, email FROM identities WHERE account_id = $1 ORDER BY provider",
    )
    .bind(acct)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(Me {
        account_id: acct,
        needs_handle: handle.is_none(),
        handle,
        identities,
    }))
}

#[derive(Deserialize)]
pub struct HandleReq {
    handle: String,
}

fn valid_handle(h: &str) -> bool {
    (3..=20).contains(&h.len())
        && h.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && !RESERVED_HANDLES.contains(&h.to_ascii_lowercase().as_str())
}

/// Set once. Changing handles later would enable impersonation churn, so it's deliberately not offered yet.
pub async fn set_handle(
    State(s): State<AppState>,
    Account(acct): Account,
    Json(r): Json<HandleReq>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !valid_handle(&r.handle) {
        return Err(ApiError::BadRequest(
            "handle must be 3-20 letters, digits or underscores",
        ));
    }
    let res = sqlx::query("UPDATE accounts SET handle = $2 WHERE id = $1 AND handle IS NULL")
        .bind(acct)
        .bind(&r.handle)
        .execute(&s.db)
        .await;
    match res {
        Ok(r2) if r2.rows_affected() == 1 => Ok(Json(serde_json::json!({ "handle": r.handle }))),
        Ok(_) => Err(ApiError::Taken("handle already set")),
        Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("23505") => {
            Err(ApiError::Taken("handle taken"))
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn unlink(
    State(s): State<AppState>,
    Account(acct): Account,
    Path(provider): Path<String>,
) -> Result<axum::http::StatusCode, ApiError> {
    let mut tx = s.db.begin().await?;
    // Serialise concurrent unlinks so two requests can't each see "2 left" and strand the account.
    sqlx::query("SELECT id FROM accounts WHERE id = $1 FOR UPDATE")
        .bind(acct)
        .fetch_one(&mut *tx)
        .await?;
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM identities WHERE account_id = $1")
        .bind(acct)
        .fetch_one(&mut *tx)
        .await?;
    if n <= 1 {
        return Err(ApiError::Taken("cannot remove your only login"));
    }
    let r = sqlx::query("DELETE FROM identities WHERE account_id = $1 AND provider = $2")
        .bind(acct)
        .bind(&provider)
        .execute(&mut *tx)
        .await?;
    if r.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    tx.commit().await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct DeleteReq {
    /// Your handle, or the word DELETE if you haven't picked one.
    confirm: String,
}

/// Erases the account and everything hanging off it (saves, seeds, runs, sessions).
pub async fn delete_account(
    State(s): State<AppState>,
    Account(acct): Account,
    Json(r): Json<DeleteReq>,
) -> Result<axum::http::StatusCode, ApiError> {
    let want = handle_of(&s.db, acct)
        .await?
        .unwrap_or_else(|| "DELETE".into());
    if !r.confirm.eq_ignore_ascii_case(&want) {
        return Err(ApiError::BadRequest("confirmation does not match"));
    }
    sqlx::query("DELETE FROM accounts WHERE id = $1")
        .bind(acct)
        .execute(&s.db)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}
