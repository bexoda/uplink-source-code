use sqlx::postgres::PgPoolOptions;
use ur_save::Key;
use ur_server::auth::providers_from_env;
use ur_server::{migrate, router, AppState};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let bind = std::env::var("BIND").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&url)
        .await
        .expect("connect to postgres");
    migrate(&db).await.expect("run migrations");
    let server_key = parse_key(
        &std::env::var("UPLINK_RUST_SERVER_KEY")
            .expect("UPLINK_RUST_SERVER_KEY is required (64 hex chars)"),
    );
    let public_url = std::env::var("UPLINK_RUST_PUBLIC_URL")
        .expect("UPLINK_RUST_PUBLIC_URL is required, e.g. https://api.example.com");
    let web_origins = std::env::var("UPLINK_RUST_WEB_ORIGINS")
        .map(|v| {
            v.split(',')
                .map(|o| o.trim().trim_end_matches('/').to_string())
                .filter(|o| !o.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let mut providers = providers_from_env();
    let dev_login = std::env::var("UPLINK_RUST_DEV_LOGIN").is_ok_and(|v| v == "1");
    if dev_login {
        if ur_server::devidp::allowed(&public_url) {
            tracing::warn!(
                "development sign-in is ON: anyone who can reach this server can sign in as anyone"
            );
            providers.insert(
                ur_server::devidp::NAME.into(),
                ur_server::devidp::config(&public_url),
            );
        } else {
            panic!(
                "UPLINK_RUST_DEV_LOGIN=1 is only allowed when UPLINK_RUST_PUBLIC_URL is loopback"
            );
        }
    }
    if providers.is_empty() {
        tracing::warn!("no OAuth providers configured; nobody can log in");
    }
    let state = AppState::new(db, server_key, providers, public_url, web_origins);
    let mut app = router(state, std::env::var("UPLINK_RUST_ALLOWED_ORIGIN").ok());
    if dev_login {
        app = app.merge(ur_server::devidp::routes());
    }
    let listener = tokio::net::TcpListener::bind(&bind).await.expect("bind");
    tracing::info!("listening on {bind}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .unwrap();
}

fn parse_key(hex: &str) -> Key {
    assert!(
        hex.len() == 64 && hex.is_ascii(),
        "UPLINK_RUST_SERVER_KEY must be 64 hex characters"
    );
    let mut b = [0u8; 32];
    for (i, byte) in b.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .expect("UPLINK_RUST_SERVER_KEY must be hex");
    }
    Key::from_bytes(b)
}
