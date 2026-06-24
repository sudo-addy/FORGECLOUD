mod api;
pub mod db;
mod engine;

use axum::{routing::{get, post}, Router};
use axum::extract::DefaultBodyLimit;
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

    // Read database URL
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/forgecloud".to_string());

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
    // Falls back to a deterministic dev-only key when unset.
    let master_key: [u8; 32] = match std::env::var("MASTER_KEY") {
        Ok(hex_str) => {
            let decoded = hex::decode(hex_str.trim())
                .expect("MASTER_KEY must be valid hex (64 hex chars = 32 bytes)");
            let mut key = [0u8; 32];
            assert!(decoded.len() == 32, "MASTER_KEY must decode to exactly 32 bytes");
            key.copy_from_slice(&decoded);
            info!("Loaded MASTER_KEY from environment");
            key
        }
        Err(_) => {
            let key = [0xABu8; 32]; // ⚠ Dev-only fallback — never use in production
            tracing::warn!("MASTER_KEY not set — using insecure development default");
            key
        }
    };

    let app_state = AppState { db, storage, master_key };

    // Build the Axum router
    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/v1/files/upload", post(api::upload::upload_handler))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024)) // 50MB body limit for uploads
        .route("/v1/files/download/:id", get(api::download::download_file_handler))
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
