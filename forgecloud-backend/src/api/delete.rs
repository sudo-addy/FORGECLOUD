use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

use crate::AppState;

#[derive(Serialize)]
pub struct DeleteResponse {
    pub message: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

fn err_response(status: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error: msg.into() }))
}

#[derive(FromRow)]
struct ChunkBackendRecord {
    backend_chunk_id: String,
}

pub async fn delete_file_handler(
    Path(file_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<DeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    // First, check if file exists
    let file_exists: (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM files WHERE id = $1)")
        .bind(file_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| {
            err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?;

    if !file_exists.0 {
        return Err(err_response(StatusCode::NOT_FOUND, "File not found"));
    }

    // Get all chunks for physical cleanup
    let chunks = sqlx::query_as::<_, ChunkBackendRecord>(
        "SELECT backend_chunk_id FROM chunks WHERE file_id = $1",
    )
    .bind(file_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("DB error: {}", e),
        )
    })?;

    // Attempt to delete chunks from physical storage (Option B approach)
    // We log warnings if they fail, but continue so we can at least prune the DB.
    // Note: Telegram storage will safely log a warning and return Ok(()) due to its immutable architecture.
    for chunk in chunks {
        if let Err(e) = state.storage.delete_chunk(&chunk.backend_chunk_id).await {
            tracing::warn!(
                "Failed to delete chunk {} from physical storage: {}",
                chunk.backend_chunk_id,
                e
            );
        }
    }

    // Delete from database (Cascades to chunks table)
    sqlx::query("DELETE FROM files WHERE id = $1")
        .bind(file_id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to delete from DB: {}", e),
            )
        })?;

    Ok(Json(DeleteResponse {
        message: "File securely unlinked and deleted".to_string(),
    }))
}
