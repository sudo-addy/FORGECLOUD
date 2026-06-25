mod api;
pub mod db;
mod engine;

use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{IntoResponse, Response};
use axum::{
    routing::{delete, get, patch, post},
    Router,
};
use sqlx::PgPool;
use uuid::Uuid;

use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tower_http::cors::CorsLayer;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::engine::storage::{LocalStorageProvider, StorageProvider, TelegramStorageProvider};

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
            axum::Json(
                serde_json::json!({ "error": "Unauthorized: Invalid or missing x-api-key header" }),
            ),
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
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    info!("Starting forgecloud-backend...");

    // Read database URL — must be set; no insecure fallback allowed
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set (e.g. in .env)");

    // Initialize the database pool
    let db = db::init_db(&database_url).await?;

    // Run migrations automatically (runtime path-based, no compile-time DATABASE_URL needed)
    let migrator = sqlx::migrate::Migrator::new(std::path::Path::new("./migrations")).await?;
    migrator.run(&db).await?;
    info!("Database migrations complete");

    // Read storage type, default to telegram for serverless mode
    let storage_type = std::env::var("STORAGE_TYPE").unwrap_or_else(|_| "telegram".to_string());
    let storage: Arc<dyn StorageProvider> = if storage_type.to_lowercase() == "local" {
        let local_dir =
            std::env::var("LOCAL_STORAGE_DIR").unwrap_or_else(|_| "./chunks".to_string());
        let local_provider = LocalStorageProvider::new(&local_dir).await?;
        info!("Local storage provider initialized at {:?}", local_dir);
        Arc::new(local_provider)
    } else {
        // Initialize the Telegram storage backend
        let bot_token =
            std::env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN must be set");
        let chat_id = std::env::var("TELEGRAM_CHAT_ID").expect("TELEGRAM_CHAT_ID must be set");
        let api_base_url = std::env::var("TELEGRAM_API_URL")
            .unwrap_or_else(|_| "https://api.telegram.org".to_string());

        let storage_provider = TelegramStorageProvider::new(bot_token, chat_id, api_base_url);
        info!("Telegram storage provider initialized (public API)");
        Arc::new(storage_provider)
    };

    // Parse the master encryption key from a hex-encoded env var.
    // MASTER_KEY must be set — no insecure fallback is permitted.
    let master_key: [u8; 32] = {
        let hex_str =
            std::env::var("MASTER_KEY").expect("MASTER_KEY must be set (64 hex chars = 32 bytes)");
        let decoded = hex::decode(hex_str.trim())
            .expect("MASTER_KEY must be valid hex (64 hex chars = 32 bytes)");
        assert!(
            decoded.len() == 32,
            "MASTER_KEY must decode to exactly 32 bytes"
        );
        let mut key = [0u8; 32];
        key.copy_from_slice(&decoded);
        info!("Loaded MASTER_KEY from environment");
        key
    };

    let api_key = std::env::var("API_KEY").expect("API_KEY must be set for secure access");

    // Run startup recovery process
    if let Err(e) = run_startup_recovery(&db).await {
        tracing::error!("Startup recovery failed: {}", e);
    } else {
        // Run an immediate cleanup pass for any abandoned sessions on startup
        if let Err(e) = perform_sessions_cleanup(&db, &storage).await {
            tracing::error!("Startup cleanup pass failed: {}", e);
        }
    }

    // Start background cleanup worker
    start_background_cleanup_worker(db.clone(), storage.clone());

    let app_state = AppState {
        db,
        storage,
        master_key,
        api_key,
    };

    let allowed_origins_str = std::env::var("ALLOWED_ORIGINS").unwrap_or_else(|_| {
        "http://localhost:3000,http://localhost:3001,http://localhost:3002".to_string()
    });

    let allowed_origins: Vec<HeaderValue> = allowed_origins_str
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<HeaderValue>()
                .expect("Invalid ALLOWED_ORIGINS URI")
        })
        .collect();

    let cors = CorsLayer::new()
        .allow_origin(allowed_origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::DELETE,
            Method::PATCH,
            Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT,
            "x-api-key".parse::<HeaderName>().unwrap(),
        ])
        .expose_headers([axum::http::header::CONTENT_DISPOSITION]);

    // Set up rate limiting: 5 requests per second (200ms interval), burst 10
    // Uses default IP-based KeyExtractor.
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_millisecond(200)
            .burst_size(10)
            .finish()
            .unwrap(),
    );

    // Build the Axum router
    let files_router = Router::new()
        .route("/", get(api::list::list_files_handler))
        .route(
            "/upload",
            post(api::upload::upload_handler).layer(DefaultBodyLimit::disable()),
        )
        .route(
            "/upload/session",
            post(api::upload::start_upload_session_handler),
        )
        .route(
            "/upload/session/:id",
            get(api::upload::get_upload_session_handler),
        )
        .route(
            "/upload/session/:id/chunk",
            post(api::upload::upload_session_chunk_handler).layer(DefaultBodyLimit::disable()),
        )
        .route(
            "/upload/session/:id/commit",
            post(api::upload::commit_upload_session_handler),
        )
        .route(
            "/upload/session/:id/abort",
            post(api::upload::abort_upload_session_handler),
        )
        .route(
            "/upload/session/:id/pause",
            post(api::upload::pause_upload_session_handler),
        )
        .route(
            "/upload/session/:id/resume",
            post(api::upload::resume_upload_session_handler),
        )
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024)) // 50MB body limit for other endpoints above
        .route("/download/:id", get(api::download::download_file_handler))
        .route(
            "/:id",
            delete(api::delete::delete_file_handler).patch(api::list::update_file_handler),
        )
        .route(
            "/:id/shares",
            post(api::shares::create_share_handler).get(api::shares::list_shares_handler),
        )
        .route_layer(from_fn_with_state(app_state.clone(), api_key_auth))
        .layer(GovernorLayer {
            config: governor_conf.clone(),
        });

    let folders_router = Router::new()
        .route(
            "/",
            post(api::folders::create_folder_handler).get(api::folders::list_folders_handler),
        )
        .route(
            "/:id",
            patch(api::folders::update_folder_handler).delete(api::folders::delete_folder_handler),
        )
        .route_layer(from_fn_with_state(app_state.clone(), api_key_auth))
        .layer(GovernorLayer {
            config: governor_conf.clone(),
        });

    let shares_protected = Router::new()
        .route("/:id", delete(api::shares::delete_share_handler))
        .route_layer(from_fn_with_state(app_state.clone(), api_key_auth));

    let shares_public = Router::new()
        .route("/public/:token", get(api::shares::get_share_info_handler))
        .route(
            "/public/:token/download",
            get(api::shares::download_share_handler),
        );

    let shares_router = shares_protected.merge(shares_public).layer(GovernorLayer {
        config: governor_conf.clone(),
    });

    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .nest("/v1/files", files_router)
        .nest("/v1/folders", folders_router)
        .nest("/v1/shares", shares_router)
        .layer(cors)
        .with_state(app_state);

    // Bind the server to 0.0.0.0:3000
    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    info!("Listening on {}", listener.local_addr()?);

    // Run the server with graceful shutdown signal handler
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
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

async fn run_startup_recovery(db: &PgPool) -> Result<(), sqlx::Error> {
    let affected = sqlx::query(
        "UPDATE upload_sessions \
         SET status = 'Abandoned', updated_at = CURRENT_TIMESTAMP \
         WHERE status IN ('Created', 'Uploading', 'PendingCommit')",
    )
    .execute(db)
    .await?;
    info!(
        "Startup recovery: Marked {} unfinished upload sessions as Abandoned",
        affected.rows_affected()
    );
    Ok(())
}

fn start_background_cleanup_worker(db: PgPool, storage: Arc<dyn StorageProvider>) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(30 * 60)).await; // Run every 30 minutes
            info!("Running background upload sessions cleanup worker...");
            if let Err(e) = perform_sessions_cleanup(&db, &storage).await {
                tracing::error!("Error in background cleanup worker: {}", e);
            }
        }
    });
}

async fn perform_sessions_cleanup(
    db: &PgPool,
    storage: &Arc<dyn StorageProvider>,
) -> Result<(), anyhow::Error> {
    use sqlx::Row;
    // Find all sessions in 'Abandoned' or 'Failed' status,
    // OR 'Created'/'Uploading'/'Paused' sessions that haven't had any activity for more than 2 hours.
    let sessions = sqlx::query(
        "SELECT id FROM upload_sessions \
         WHERE status IN ('Abandoned', 'Failed') \
            OR (status IN ('Created', 'Uploading', 'Paused') AND last_activity_at < CURRENT_TIMESTAMP - INTERVAL '2 hours')",
    )
    .fetch_all(db)
    .await?;

    for session_row in sessions {
        let session_id: Uuid = session_row.get("id");
        info!(
            "Cleaning up expired/abandoned upload session: {}",
            session_id
        );

        // Fetch all pending chunks for this session
        let chunks = sqlx::query(
            "SELECT backend_chunk_id FROM pending_session_chunks WHERE session_id = $1",
        )
        .bind(session_id)
        .fetch_all(db)
        .await?;

        // Delete chunks physically from storage
        for chunk in chunks {
            let backend_chunk_id: String = chunk.get("backend_chunk_id");
            let _ = storage.delete_chunk(&backend_chunk_id).await;
        }

        // Delete the session (cascades to pending_session_chunks)
        sqlx::query("DELETE FROM upload_sessions WHERE id = $1")
            .bind(session_id)
            .execute(db)
            .await?;
    }
    Ok(())
}
