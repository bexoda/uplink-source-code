use gl_save::Key;
use gl_server::{migrate, router, AppState};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

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
        &std::env::var("GL_SERVER_KEY").expect("GL_SERVER_KEY is required (64 hex chars)"),
    );
    let state = AppState {
        db,
        registration_key: std::env::var("GL_REGISTRATION_KEY").ok(),
        server_key: Arc::new(server_key),
    };
    let app = router(state, std::env::var("GL_ALLOWED_ORIGIN").ok());
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
        "GL_SERVER_KEY must be 64 hex characters"
    );
    let mut b = [0u8; 32];
    for (i, byte) in b.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).expect("GL_SERVER_KEY must be hex");
    }
    Key::from_bytes(b)
}
