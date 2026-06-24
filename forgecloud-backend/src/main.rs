mod api;
pub mod db;
mod engine;

use axum::{routing::{get, post}, Router};
use axum::extract::{DefaultBodyLimit, State, Request};
use axum::http::{StatusCode, HeaderMap};
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{IntoResponse, Response};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::engine::storage::{StorageProvider, TelegramStorageProvider, LocalStorageProvider};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub storage: Arc<dyn StorageProvider>,
    pub master_key: [u8; 32],
    pub api_key: String,
}

async fn api_key_auth(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, impl IntoResponse> {
    let api_key = headers.get("x-api-key").and_then(|v| v.to_str().ok());
    
    if api_key != Some(&state.api_key) {
        return Err((
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({ "error": "Unauthorized: Invalid or missing x-api-key header" }))
        ));
    }
    
    Ok(next.run(request).await)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables from .env file (silently ignore if file not present)
    dotenvy::dotenv().ok();

    // Initialize tracing registry with EnvFilter defaulting to info
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    info!("Starting forgecloud-backend...");

    // Read database URL — must be set; no insecure fallback allowed
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set (e.g. in .env)");

    // Initialize the database pool
    let db = db::init_db(&database_url).await?;

    // Run migrations automatically (runtime path-based, no compile-time DATABASE_URL needed)
    let migrator = sqlx::migrate::Migrator::new(std::path::Path::new("./migrations")).await?;
    migrator.run(&db).await?;
    info!("Database migrations complete");

    // Read storage type, default to telegram for serverless mode
    let storage_type = std::env::var("STORAGE_TYPE").unwrap_or_else(|_| "telegram".to_string());
    let storage: Arc<dyn StorageProvider> = if storage_type.to_lowercase() == "local" {
        let local_dir = std::env::var("LOCAL_STORAGE_DIR").unwrap_or_else(|_| "./chunks".to_string());
        let local_provider = LocalStorageProvider::new(&local_dir).await?;
        info!("Local storage provider initialized at {:?}", local_dir);
        Arc::new(local_provider)
    } else {
        // Initialize the Telegram storage backend
        let bot_token = std::env::var("TELEGRAM_BOT_TOKEN")
            .expect("TELEGRAM_BOT_TOKEN must be set");
        let chat_id = std::env::var("TELEGRAM_CHAT_ID")
            .expect("TELEGRAM_CHAT_ID must be set");
        let api_base_url = std::env::var("TELEGRAM_API_URL")
            .unwrap_or_else(|_| "https://api.telegram.org".to_string());

        let storage_provider = TelegramStorageProvider::new(bot_token, chat_id, api_base_url);
        info!("Telegram storage provider initialized (public API)");
        Arc::new(storage_provider)
    };

    // Parse the master encryption key from a hex-encoded env var.
    // MASTER_KEY must be set — no insecure fallback is permitted.
    let master_key: [u8; 32] = {
        let hex_str = std::env::var("MASTER_KEY")
            .expect("MASTER_KEY must be set (64 hex chars = 32 bytes)");
        let decoded = hex::decode(hex_str.trim())
            .expect("MASTER_KEY must be valid hex (64 hex chars = 32 bytes)");
        assert!(decoded.len() == 32, "MASTER_KEY must decode to exactly 32 bytes");
        let mut key = [0u8; 32];
        key.copy_from_slice(&decoded);
        info!("Loaded MASTER_KEY from environment");
        key
    };

    let api_key = std::env::var("API_KEY").expect("API_KEY must be set for secure access");

    let app_state = AppState { db, storage, master_key, api_key };

    // Build the Axum router
    let files_router = Router::new()
        .route("/", get(api::list::list_files_handler))
        .route("/upload", post(api::upload::upload_handler))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024)) // 50MB body limit for uploads
        .route("/download/:id", get(api::download::download_file_handler))
        .route_layer(from_fn_with_state(app_state.clone(), api_key_auth));

    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .nest("/v1/files", files_router)
        .with_state(app_state);

    // Bind the server to 0.0.0.0:3000
    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    info!("Listening on {}", listener.local_addr()?);

    // Run the server with graceful shutdown signal handler
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received, starting graceful shutdown");
}
