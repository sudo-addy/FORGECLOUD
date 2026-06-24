use std::io::{Error, ErrorKind};

use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    Json,
};
use futures_util::StreamExt;
use serde::Serialize;
use tracing::instrument;
use uuid::Uuid;

use crate::AppState;
use crate::engine::chunker::process_upload_stream;

#[derive(Serialize)]
pub struct UploadResponse {
    pub file_id: Uuid,
    pub name: String,
    pub total_size: i64,
    pub chunks_count: usize,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Helper to build a consistent error tuple.
fn err_response(status: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error: msg.into() }))
}

#[instrument(skip(state, multipart), fields(file_name, content_type))]
pub async fn upload_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, (StatusCode, Json<ErrorResponse>)> {
    let storage = state.storage;

    // Inspect multipart fields sequentially to look for a file field
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| err_response(StatusCode::BAD_REQUEST, format!("Failed to read multipart field: {e}")))?
    {
        // Only process fields that are file uploads
        let file_name = match field.file_name() {
            Some(name) => {
                tracing::Span::current().record("file_name", name);
                name.to_owned()
            }
            None => continue,
        };

        let mime_type = field.content_type().map(|ct| {
            tracing::Span::current().record("content_type", ct);
            ct.to_owned()
        });

        // Do NOT load into memory via .bytes().
        // Extract the underlying body stream and map errors to a unified std::io::Error type.
        let stream =
            field.map(|res| res.map_err(|e| Error::new(ErrorKind::Other, e.to_string())));

        // Target chunk size: 45MB (safely under Telegram public API 50MB limit)
        let target_chunk_size: u64 = 45 * 1024 * 1024;

        // Pass the live network stream directly into the chunker engine
        let chunks = process_upload_stream(stream, storage, target_chunk_size, &state.master_key)
            .await
            .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("Upload processing failed: {e}")))?;

        // ---------------------------------------------------------------
        // Database insertion inside a transaction
        // ---------------------------------------------------------------
        let file_id = Uuid::new_v4();
        let total_size: i64 = chunks.iter().map(|c| c.size_bytes as i64).sum();

        let mut tx = state.db.begin().await.map_err(|e| {
            err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to begin transaction: {e}"))
        })?;

        // Insert file record
        sqlx::query(
            "INSERT INTO files (id, name, total_size, mime_type) VALUES ($1, $2, $3, $4)"
        )
        .bind(file_id)
        .bind(&file_name)
        .bind(total_size)
        .bind(mime_type.as_deref())
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to insert file record: {e}"))
        })?;

        // Insert chunk records (0-indexed)
        for (idx, chunk) in chunks.iter().enumerate() {
            let chunk_id = Uuid::new_v4();
            let chunk_number = idx as i32;

            sqlx::query(
                "INSERT INTO chunks (id, file_id, chunk_number, backend_chunk_id, size_bytes) VALUES ($1, $2, $3, $4, $5)"
            )
            .bind(chunk_id)
            .bind(file_id)
            .bind(chunk_number)
            .bind(&chunk.backend_chunk_id)
            .bind(chunk.size_bytes as i64)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                err_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to insert chunk record: {e}"),
                )
            })?;
        }

        // Commit only when all records are verified
        tx.commit().await.map_err(|e| {
            err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to commit transaction: {e}"))
        })?;

        return Ok(Json(UploadResponse {
            file_id,
            name: file_name,
            total_size,
            chunks_count: chunks.len(),
        }));
    }

    Err(err_response(
        StatusCode::BAD_REQUEST,
        "No file field found in the multipart payload",
    ))
}
