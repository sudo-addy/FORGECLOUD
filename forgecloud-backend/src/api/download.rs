use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::BytesMut;
use futures_util::StreamExt;
use std::io::Error;
use uuid::Uuid;

use crate::engine::crypto;
use crate::AppState;

use sqlx::FromRow;

/// AES-256-GCM overhead: 12-byte nonce + 16-byte authentication tag.
const CRYPTO_OVERHEAD_PER_CHUNK: i64 = 12 + 16;

#[derive(FromRow)]
pub struct FileRecord {
    pub name: String,
    pub total_size: i64,
    pub mime_type: Option<String>,
}

#[derive(FromRow)]
pub struct ChunkRecord {
    pub backend_chunk_id: String,
    pub size_bytes: i64,
}

pub async fn download_file_handler(
    Path(file_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, Response> {
    let file_record = sqlx::query_as::<_, FileRecord>(
        r#"
        SELECT name, total_size, mime_type
        FROM files
        WHERE id = $1
        "#,
    )
    .bind(file_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database error: {}", e),
        )
            .into_response()
    })?;

    let file_record = match file_record {
        Some(f) => f,
        None => return Err((StatusCode::NOT_FOUND, "File not found").into_response()),
    };

    let chunks = sqlx::query_as::<_, ChunkRecord>(
        r#"
        SELECT backend_chunk_id, size_bytes
        FROM chunks
        WHERE file_id = $1
        ORDER BY chunk_number ASC
        "#,
    )
    .bind(file_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database error: {}", e),
        )
            .into_response()
    })?;

    stream_file_response(file_record, chunks, state.storage, state.master_key)
}

pub fn stream_file_response(
    file_record: FileRecord,
    chunks: Vec<ChunkRecord>,
    storage: std::sync::Arc<dyn crate::engine::storage::StorageProvider>,
    master_key: [u8; 32],
) -> Result<Response, Response> {
    // Calculate the total plaintext size by subtracting crypto overhead per chunk.
    let plaintext_total_size: i64 = chunks
        .iter()
        .map(|c| c.size_bytes - CRYPTO_OVERHEAD_PER_CHUNK)
        .sum();

    // For each chunk: download the encrypted blob, collect it fully, decrypt,
    // then emit the plaintext bytes into the response stream.
    let stream = futures_util::stream::iter(chunks).then(move |chunk| {
        let storage = storage.clone();
        let key = master_key;
        async move {
            // Download the encrypted chunk as a byte stream
            let mut chunk_stream = storage
                .download_chunk(&chunk.backend_chunk_id)
                .await
                .map_err(|e| Error::other(format!("Storage error: {}", e)))?;

            // Collect the full encrypted blob into memory (bounded by chunk size + overhead)
            let mut encrypted = BytesMut::with_capacity(chunk.size_bytes as usize);
            while let Some(piece) = chunk_stream.next().await {
                let bytes = piece?;
                encrypted.extend_from_slice(&bytes);
            }

            // Decrypt the chunk
            let plaintext = crypto::decrypt_chunk(&encrypted, &key)
                .map_err(|e| Error::other(format!("Decryption error: {}", e)))?;

            Ok::<_, Error>(bytes::Bytes::from(plaintext))
        }
    });

    let body = Body::from_stream(stream);

    let mime_type = file_record
        .mime_type
        .unwrap_or_else(|| "application/octet-stream".to_string());

    let content_disposition = format!("attachment; filename=\"{}\"", file_record.name);

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, mime_type),
            (header::CONTENT_LENGTH, plaintext_total_size.to_string()),
            (header::CONTENT_DISPOSITION, content_disposition),
        ],
        body,
    )
        .into_response())
}
