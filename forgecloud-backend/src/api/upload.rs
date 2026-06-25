use std::io::Error;

use axum::extract::Query;
use axum::{
    extract::{Multipart, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, Row};
use tracing::instrument;
use uuid::Uuid;

use crate::engine::chunker::process_upload_stream;
use crate::AppState;

#[derive(Serialize)]
pub struct UploadResponse {
    pub file_id: Uuid,
    pub name: String,
    pub total_size: i64,
    pub chunks_count: usize,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadQuery {
    pub folder_id: Option<Uuid>,
}

/// Helper to build a consistent error tuple.
fn err_response(status: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error: msg.into() }))
}

#[instrument(skip(state, multipart), fields(file_name, content_type))]
pub async fn upload_handler(
    State(state): State<AppState>,
    Query(query): Query<UploadQuery>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, (StatusCode, Json<ErrorResponse>)> {
    let storage = state.storage;

    // Inspect multipart fields sequentially to look for a file field
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        err_response(
            StatusCode::BAD_REQUEST,
            format!("Failed to read multipart field: {e}"),
        )
    })? {
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

        // Compute hash on-the-fly for single request upload
        let mut hasher = Sha256::new();
        let stream = field.map(move |res| {
            res.inspect(|bytes| {
                hasher.update(bytes);
            })
            .map_err(|e| Error::other(e.to_string()))
        });

        // Target chunk size: 45MB (safely under Telegram public API 50MB limit)
        let target_chunk_size: u64 = 45 * 1024 * 1024;

        // Pass the live network stream directly into the chunker engine
        let chunks = process_upload_stream(stream, storage, target_chunk_size, &state.master_key)
            .await
            .map_err(|e| {
                err_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Upload processing failed: {e}"),
                )
            })?;

        // ---------------------------------------------------------------
        // Database insertion inside a transaction
        // ---------------------------------------------------------------
        let file_id = Uuid::new_v4();
        let total_size: i64 = chunks.iter().map(|c| c.size_bytes as i64).sum();

        let mut tx = state.db.begin().await.map_err(|e| {
            err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to begin transaction: {e}"),
            )
        })?;

        // Insert file record
        sqlx::query(
            "INSERT INTO files (id, name, total_size, mime_type, folder_id) VALUES ($1, $2, $3, $4, $5)"
        )
        .bind(file_id)
        .bind(&file_name)
        .bind(total_size)
        .bind(mime_type.as_deref())
        .bind(query.folder_id)
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
            err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to commit transaction: {e}"),
            )
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

// ---------------------------------------------------------------------------
// V2 Session-based Upload API Handlers
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct StartSessionRequest {
    pub name: String,
    pub total_size: i64,
    pub mime_type: Option<String>,
    pub folder_id: Option<Uuid>,
    pub sha256_hash: Option<String>,
}

#[derive(Serialize)]
pub struct StartSessionResponse {
    pub session_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct ChunkUploadQuery {
    pub chunk_number: i32,
}

#[derive(Serialize)]
pub struct CommitSessionResponse {
    pub file_id: Uuid,
    pub name: String,
    pub total_size: i64,
    pub chunks_count: usize,
}

#[derive(Serialize)]
pub struct SessionInspectResponse {
    pub session_id: Uuid,
    pub name: String,
    pub total_size: i64,
    pub mime_type: Option<String>,
    pub status: String,
    pub chunks_uploaded: usize,
    pub total_chunks_expected: usize,
    pub uploaded_bytes: i64,
    pub percent_complete: f64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_activity_at: chrono::DateTime<chrono::Utc>,
    pub estimated_remaining_time_secs: Option<i64>,
}

#[derive(FromRow)]
struct SessionInfo {
    name: String,
    total_size: i64,
    mime_type: Option<String>,
    folder_id: Option<Uuid>,
    status: String,
    sha256_hash: Option<String>,
    owner_api_key: String,
}

#[derive(FromRow)]
struct PendingChunkInfo {
    backend_chunk_id: String,
    size_bytes: i64,
    chunk_number: i32,
    chunk_sha256: String,
}

pub async fn start_upload_session_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<StartSessionRequest>,
) -> Result<Json<StartSessionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let session_id = Uuid::new_v4();
    let api_key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();

    sqlx::query(
        "INSERT INTO upload_sessions (id, name, total_size, mime_type, folder_id, status, sha256_hash, owner_api_key) \
         VALUES ($1, $2, $3, $4, $5, 'Created', $6, $7)"
    )
    .bind(session_id)
    .bind(&payload.name)
    .bind(payload.total_size)
    .bind(payload.mime_type.as_deref())
    .bind(payload.folder_id)
    .bind(payload.sha256_hash.as_deref())
    .bind(&api_key)
    .execute(&state.db)
    .await
    .map_err(|e| {
        err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create upload session: {e}"),
        )
    })?;

    Ok(Json(StartSessionResponse { session_id }))
}

#[instrument(skip(state, multipart), fields(session_id))]
pub async fn upload_session_chunk_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(session_id): axum::extract::Path<Uuid>,
    Query(query): Query<ChunkUploadQuery>,
    mut multipart: Multipart,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let api_key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();

    // 1. Verify session exists and is owned by the request API key
    let session_row =
        sqlx::query("SELECT owner_api_key, status FROM upload_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| {
                err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))
            })?;

    let (owner_key, status) = match session_row {
        Some(row) => (
            row.get::<String, _>("owner_api_key"),
            row.get::<String, _>("status"),
        ),
        None => return Err(err_response(StatusCode::NOT_FOUND, "Session not found")),
    };

    if owner_key != api_key {
        return Err(err_response(
            StatusCode::FORBIDDEN,
            "Forbidden: Session owner mismatch",
        ));
    }

    if status != "Created" && status != "Uploading" && status != "Paused" {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            format!("Invalid session status: {}", status),
        ));
    }

    // 2. Process the chunk payload
    let mut chunk_info = None;
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        err_response(
            StatusCode::BAD_REQUEST,
            format!("Failed to read multipart field: {e}"),
        )
    })? {
        let field_name = field.name().unwrap_or_default();
        if field_name != "file" && field.file_name().is_none() {
            continue;
        }

        // Read the chunk plaintext bytes
        let data = field.bytes().await.map_err(|e| {
            err_response(
                StatusCode::BAD_REQUEST,
                format!("Failed to read chunk data: {e}"),
            )
        })?;

        let size_bytes = data.len() as i64;

        // Compute chunk plaintext hash
        let mut chunk_hasher = Sha256::new();
        chunk_hasher.update(&data);
        let chunk_sha256 = hex::encode(chunk_hasher.finalize());

        // 3. Resume Conflict Resolution
        let existing_chunk = sqlx::query(
            "SELECT backend_chunk_id, chunk_sha256, retry_count FROM pending_session_chunks \
             WHERE session_id = $1 AND chunk_number = $2",
        )
        .bind(session_id)
        .bind(query.chunk_number)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| {
            err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to query existing chunks: {e}"),
            )
        })?;

        let mut retry_count = 0;
        if let Some(ref ec) = existing_chunk {
            let existing_sha: String = ec.get("chunk_sha256");
            if existing_sha == chunk_sha256 {
                // Duplicate chunk exists with same content, ignore re-upload
                return Ok(StatusCode::OK);
            }
            // Mismatch: retrieve previous retry count and overwrite
            retry_count = ec.get::<i32, _>("retry_count") + 1;
            let old_backend_id: String = ec.get("backend_chunk_id");
            let storage = state.storage.clone();
            tokio::spawn(async move {
                let _ = storage.delete_chunk(&old_backend_id).await;
            });
        }

        // Encrypt on-the-fly
        let encrypted =
            crate::engine::crypto::encrypt_chunk(&data, &state.master_key).map_err(|e| {
                err_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Encryption failed: {e}"),
                )
            })?;

        let backend_chunk_id = Uuid::new_v4().to_string();
        let size = encrypted.len() as u64;

        // Upload ciphertext to storage provider
        let stream = Box::pin(std::io::Cursor::new(encrypted));
        let info = state
            .storage
            .upload_chunk(&backend_chunk_id, stream, size)
            .await
            .map_err(|e| {
                err_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Storage upload failed: {e}"),
                )
            })?;

        chunk_info = Some((info, size_bytes, chunk_sha256, retry_count));
        break;
    }

    let (info, size_bytes, chunk_sha256, retry_count) = match chunk_info {
        Some(val) => val,
        None => {
            return Err(err_response(
                StatusCode::BAD_REQUEST,
                "No chunk content found",
            ))
        }
    };

    let storage_provider = std::env::var("STORAGE_TYPE").unwrap_or_else(|_| "telegram".to_string());

    // 4. Save to pending_session_chunks (with instant cleanup on failure)
    let chunk_uuid = Uuid::new_v4();
    if let Err(e) = sqlx::query(
        "INSERT INTO pending_session_chunks (id, session_id, chunk_number, backend_chunk_id, size_bytes, encrypted_size, chunk_sha256, storage_provider, retry_count) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
         ON CONFLICT (session_id, chunk_number) DO UPDATE SET \
            backend_chunk_id = EXCLUDED.backend_chunk_id, \
            size_bytes = EXCLUDED.size_bytes, \
            encrypted_size = EXCLUDED.encrypted_size, \
            chunk_sha256 = EXCLUDED.chunk_sha256, \
            storage_provider = EXCLUDED.storage_provider, \
            retry_count = EXCLUDED.retry_count, \
            upload_timestamp = CURRENT_TIMESTAMP"
    )
    .bind(chunk_uuid)
    .bind(session_id)
    .bind(query.chunk_number)
    .bind(&info.backend_chunk_id)
    .bind(size_bytes)
    .bind(info.size_bytes as i64)
    .bind(&chunk_sha256)
    .bind(&storage_provider)
    .bind(retry_count)
    .execute(&state.db)
    .await {
        // Rollback: delete physical chunk immediately
        let _ = state.storage.delete_chunk(&info.backend_chunk_id).await;
        return Err(err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to record pending chunk: {e}"),
        ));
    }

    // 5. Update session status to 'Uploading' and activity timestamp
    sqlx::query(
        "UPDATE upload_sessions SET status = 'Uploading', updated_at = CURRENT_TIMESTAMP, last_activity_at = CURRENT_TIMESTAMP WHERE id = $1"
    )
    .bind(session_id)
    .execute(&state.db)
    .await
    .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to update status: {e}")))?;

    Ok(StatusCode::OK)
}

#[instrument(skip(state))]
pub async fn commit_upload_session_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(session_id): axum::extract::Path<Uuid>,
) -> Result<Json<CommitSessionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let api_key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();

    // 1. Fetch the session record
    let session = sqlx::query_as::<_, SessionInfo>(
        "SELECT name, total_size, mime_type, folder_id, status, sha256_hash, owner_api_key FROM upload_sessions WHERE id = $1"
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let session = match session {
        Some(s) => s,
        None => return Err(err_response(StatusCode::NOT_FOUND, "Session not found")),
    };

    if session.owner_api_key != api_key {
        return Err(err_response(
            StatusCode::FORBIDDEN,
            "Forbidden: Session owner mismatch",
        ));
    }

    if session.status != "Created" && session.status != "Uploading" && session.status != "Paused" {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            format!("Invalid session status: {}", session.status),
        ));
    }

    // 2. Fetch pending chunks
    let chunks = sqlx::query_as::<_, PendingChunkInfo>(
        "SELECT backend_chunk_id, size_bytes, chunk_number, chunk_sha256 FROM pending_session_chunks WHERE session_id = $1 ORDER BY chunk_number ASC"
    )
    .bind(session_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    if chunks.is_empty() {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            "No chunks uploaded for this session",
        ));
    }

    // Verify all chunk numbers exist sequentially from 0 to N-1
    for (expected_idx, chunk) in chunks.iter().enumerate() {
        if chunk.chunk_number != expected_idx as i32 {
            return Err(err_response(
                StatusCode::BAD_REQUEST,
                format!(
                    "Missing chunk number: expected {}, found {}",
                    expected_idx, chunk.chunk_number
                ),
            ));
        }
    }

    // 3. Compute Server-Side SHA-256 by concatenating chunk hashes
    let mut file_hasher = Sha256::new();
    for chunk in &chunks {
        file_hasher.update(chunk.chunk_sha256.as_bytes());
    }
    let server_sha256 = hex::encode(file_hasher.finalize());

    // 4. Validate client-supplied hash if present
    if let Some(ref client_hash) = session.sha256_hash {
        if client_hash != &server_sha256 {
            // Update session status to Failed and trigger rollback
            let _ = sqlx::query("UPDATE upload_sessions SET status = 'Failed', updated_at = CURRENT_TIMESTAMP WHERE id = $1")
                .bind(session_id)
                .execute(&state.db)
                .await;

            let storage = state.storage.clone();
            let chunk_ids: Vec<String> =
                chunks.iter().map(|c| c.backend_chunk_id.clone()).collect();
            tokio::spawn(async move {
                for cid in chunk_ids {
                    let _ = storage.delete_chunk(&cid).await;
                }
            });

            return Err(err_response(
                StatusCode::BAD_REQUEST,
                "Integrity verification failed: SHA-256 hash mismatch",
            ));
        }
    }

    // 5. Update session to 'PendingCommit'
    sqlx::query(
        "UPDATE upload_sessions SET status = 'PendingCommit', updated_at = CURRENT_TIMESTAMP WHERE id = $1"
    )
    .bind(session_id)
    .execute(&state.db)
    .await
    .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to update status: {e}")))?;

    // 6. Perform Transaction to move chunks and create file
    let file_id = Uuid::new_v4();
    let mut tx = state.db.begin().await.map_err(|e| {
        err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to begin transaction: {e}"),
        )
    })?;

    // Create file record using server_sha256 as the source of truth
    let insert_file_res = sqlx::query(
        "INSERT INTO files (id, name, total_size, mime_type, folder_id, sha256_hash) VALUES ($1, $2, $3, $4, $5, $6)"
    )
    .bind(file_id)
    .bind(&session.name)
    .bind(session.total_size)
    .bind(session.mime_type.as_deref())
    .bind(session.folder_id)
    .bind(&server_sha256)
    .execute(&mut *tx)
    .await;

    if let Err(e) = insert_file_res {
        // Rollback DB transaction & transition session status to 'Failed'
        let _ = tx.rollback().await;
        let _ = sqlx::query("UPDATE upload_sessions SET status = 'Failed', updated_at = CURRENT_TIMESTAMP WHERE id = $1")
            .bind(session_id)
            .execute(&state.db)
            .await;

        // Spawn async rollback cleanup for chunks
        let storage = state.storage.clone();
        let chunk_ids: Vec<String> = chunks.iter().map(|c| c.backend_chunk_id.clone()).collect();
        tokio::spawn(async move {
            for cid in chunk_ids {
                let _ = storage.delete_chunk(&cid).await;
            }
        });

        return Err(err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to insert file record: {e}"),
        ));
    }

    // Create final chunk records
    for chunk in &chunks {
        let chunk_uuid = Uuid::new_v4();
        let insert_chunk_res = sqlx::query(
            "INSERT INTO chunks (id, file_id, chunk_number, backend_chunk_id, size_bytes) VALUES ($1, $2, $3, $4, $5)"
        )
        .bind(chunk_uuid)
        .bind(file_id)
        .bind(chunk.chunk_number)
        .bind(&chunk.backend_chunk_id)
        .bind(chunk.size_bytes)
        .execute(&mut *tx)
        .await;

        if let Err(e) = insert_chunk_res {
            let _ = tx.rollback().await;
            let _ = sqlx::query("UPDATE upload_sessions SET status = 'Failed', updated_at = CURRENT_TIMESTAMP WHERE id = $1")
                .bind(session_id)
                .execute(&state.db)
                .await;

            let storage = state.storage.clone();
            let chunk_ids: Vec<String> =
                chunks.iter().map(|c| c.backend_chunk_id.clone()).collect();
            tokio::spawn(async move {
                for cid in chunk_ids {
                    let _ = storage.delete_chunk(&cid).await;
                }
            });

            return Err(err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to insert chunk record: {e}"),
            ));
        }
    }

    // Update session to 'Completed' inside transaction
    let update_sess_res = sqlx::query(
        "UPDATE upload_sessions SET status = 'Completed', server_sha256 = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2"
    )
    .bind(&server_sha256)
    .bind(session_id)
    .execute(&mut *tx)
    .await;

    if let Err(e) = update_sess_res {
        let _ = tx.rollback().await;
        let _ = sqlx::query("UPDATE upload_sessions SET status = 'Failed', updated_at = CURRENT_TIMESTAMP WHERE id = $1")
            .bind(session_id)
            .execute(&state.db)
            .await;

        let storage = state.storage.clone();
        let chunk_ids: Vec<String> = chunks.iter().map(|c| c.backend_chunk_id.clone()).collect();
        tokio::spawn(async move {
            for cid in chunk_ids {
                let _ = storage.delete_chunk(&cid).await;
            }
        });

        return Err(err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to finalize session status: {e}"),
        ));
    }

    tx.commit().await.map_err(|e| {
        err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to commit transaction: {e}"),
        )
    })?;

    Ok(Json(CommitSessionResponse {
        file_id,
        name: session.name,
        total_size: session.total_size,
        chunks_count: chunks.len(),
    }))
}

#[instrument(skip(state))]
pub async fn abort_upload_session_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(session_id): axum::extract::Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let api_key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();

    // 1. Verify session exists and belongs to the requesting API key
    let session = sqlx::query("SELECT owner_api_key FROM upload_sessions WHERE id = $1")
        .bind(session_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let owner_key = match session {
        Some(s) => s.get::<String, _>("owner_api_key"),
        None => return Err(err_response(StatusCode::NOT_FOUND, "Session not found")),
    };

    if owner_key != api_key {
        return Err(err_response(
            StatusCode::FORBIDDEN,
            "Forbidden: Session owner mismatch",
        ));
    }

    // 2. Fetch chunks to clean up
    let chunks_rows =
        sqlx::query("SELECT backend_chunk_id FROM pending_session_chunks WHERE session_id = $1")
            .bind(session_id)
            .fetch_all(&state.db)
            .await
            .map_err(|e| {
                err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))
            })?;

    let mut chunk_ids = Vec::new();
    for r in chunks_rows {
        let cid: String = r.get("backend_chunk_id");
        chunk_ids.push(cid);
    }

    // 3. Mark session as 'Failed'
    sqlx::query(
        "UPDATE upload_sessions SET status = 'Failed', updated_at = CURRENT_TIMESTAMP WHERE id = $1"
    )
    .bind(session_id)
    .execute(&state.db)
    .await
    .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to update status: {e}")))?;

    // 4. Immediately delete chunks physically
    for cid in chunk_ids {
        let _ = state.storage.delete_chunk(&cid).await;
    }

    // 5. Delete session chunks and session records from database
    sqlx::query("DELETE FROM upload_sessions WHERE id = $1")
        .bind(session_id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to delete session: {e}"),
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

#[instrument(skip(state))]
pub async fn get_upload_session_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(session_id): axum::extract::Path<Uuid>,
) -> Result<Json<SessionInspectResponse>, (StatusCode, Json<ErrorResponse>)> {
    let api_key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();

    // 1. Fetch session record
    let session = sqlx::query(
        "SELECT name, total_size, mime_type, status, owner_api_key, created_at, last_activity_at FROM upload_sessions WHERE id = $1"
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let s = match session {
        Some(row) => row,
        None => return Err(err_response(StatusCode::NOT_FOUND, "Session not found")),
    };

    let owner_key: String = s.get("owner_api_key");
    if owner_key != api_key {
        return Err(err_response(
            StatusCode::FORBIDDEN,
            "Forbidden: Session owner mismatch",
        ));
    }

    let name: String = s.get("name");
    let total_size: i64 = s.get("total_size");
    let mime_type: Option<String> = s.get("mime_type");
    let status: String = s.get("status");
    let created_at: chrono::DateTime<chrono::Utc> = s.get("created_at");
    let last_activity_at: chrono::DateTime<chrono::Utc> = s.get("last_activity_at");

    // 2. Fetch chunks count and total uploaded bytes
    let chunks = sqlx::query("SELECT size_bytes FROM pending_session_chunks WHERE session_id = $1")
        .bind(session_id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let chunks_uploaded = chunks.len();
    let uploaded_bytes: i64 = chunks.iter().map(|r| r.get::<i64, _>("size_bytes")).sum();

    // Target chunk size: 45MB
    let target_chunk_size = 45 * 1024 * 1024;
    let total_chunks_expected = if total_size == 0 {
        1
    } else {
        ((total_size + target_chunk_size - 1) / target_chunk_size) as usize
    };

    let percent_complete = if total_size == 0 {
        100.0
    } else {
        (uploaded_bytes as f64 / total_size as f64) * 100.0
    };

    // 3. Compute estimated remaining time (seconds)
    let elapsed = last_activity_at
        .signed_duration_since(created_at)
        .num_seconds();
    let estimated_remaining_time_secs =
        if elapsed > 5 && uploaded_bytes > 0 && uploaded_bytes < total_size {
            let speed_bytes_per_sec = uploaded_bytes as f64 / elapsed as f64;
            let remaining_bytes = total_size - uploaded_bytes;
            Some((remaining_bytes as f64 / speed_bytes_per_sec) as i64)
        } else {
            None
        };

    Ok(Json(SessionInspectResponse {
        session_id,
        name,
        total_size,
        mime_type,
        status,
        chunks_uploaded,
        total_chunks_expected,
        uploaded_bytes,
        percent_complete,
        created_at,
        last_activity_at,
        estimated_remaining_time_secs,
    }))
}

pub async fn pause_upload_session_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(session_id): axum::extract::Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let api_key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();

    let session = sqlx::query("SELECT owner_api_key, status FROM upload_sessions WHERE id = $1")
        .bind(session_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let (owner_key, status) = match session {
        Some(s) => (
            s.get::<String, _>("owner_api_key"),
            s.get::<String, _>("status"),
        ),
        None => return Err(err_response(StatusCode::NOT_FOUND, "Session not found")),
    };

    if owner_key != api_key {
        return Err(err_response(
            StatusCode::FORBIDDEN,
            "Forbidden: Session owner mismatch",
        ));
    }

    if status != "Created" && status != "Uploading" {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            format!("Cannot pause session in status: {}", status),
        ));
    }

    sqlx::query(
        "UPDATE upload_sessions SET status = 'Paused', last_activity_at = CURRENT_TIMESTAMP WHERE id = $1"
    )
    .bind(session_id)
    .execute(&state.db)
    .await
    .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to pause session: {e}")))?;

    Ok(StatusCode::OK)
}

pub async fn resume_upload_session_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(session_id): axum::extract::Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let api_key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();

    let session = sqlx::query("SELECT owner_api_key, status FROM upload_sessions WHERE id = $1")
        .bind(session_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let (owner_key, status) = match session {
        Some(s) => (
            s.get::<String, _>("owner_api_key"),
            s.get::<String, _>("status"),
        ),
        None => return Err(err_response(StatusCode::NOT_FOUND, "Session not found")),
    };

    if owner_key != api_key {
        return Err(err_response(
            StatusCode::FORBIDDEN,
            "Forbidden: Session owner mismatch",
        ));
    }

    if status != "Paused" {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            format!("Cannot resume session in status: {}", status),
        ));
    }

    sqlx::query(
        "UPDATE upload_sessions SET status = 'Uploading', last_activity_at = CURRENT_TIMESTAMP WHERE id = $1"
    )
    .bind(session_id)
    .execute(&state.db)
    .await
    .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to resume session: {e}")))?;

    Ok(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Path;

    use crate::engine::storage::{StorageProvider, StoredChunkInfo};
    use async_trait::async_trait;
    use bytes::Bytes;
    use std::pin::Pin;
    use tokio::io::AsyncRead;

    struct DummyStorage;

    #[async_trait]
    impl StorageProvider for DummyStorage {
        async fn upload_chunk(
            &self,
            chunk_id: &str,
            _stream: Pin<Box<dyn AsyncRead + Send>>,
            size: u64,
        ) -> Result<StoredChunkInfo, anyhow::Error> {
            Ok(StoredChunkInfo {
                backend_chunk_id: chunk_id.to_string(),
                size_bytes: size,
            })
        }

        async fn download_chunk(
            &self,
            _backend_chunk_id: &str,
        ) -> Result<
            Pin<Box<dyn futures_util::Stream<Item = Result<Bytes, std::io::Error>> + Send>>,
            anyhow::Error,
        > {
            unimplemented!()
        }

        async fn delete_chunk(&self, _backend_chunk_id: &str) -> Result<(), anyhow::Error> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_upload_session_db_flow() {
        dotenvy::dotenv().ok();
        let database_url = match std::env::var("DATABASE_URL") {
            Ok(url) => url,
            Err(_) => return, // Skip test if no DB
        };

        let db = sqlx::PgPool::connect(&database_url).await.unwrap();

        // Drop existing tables to ensure clean schema for test
        let _ = sqlx::query("DROP TABLE IF EXISTS pending_session_chunks CASCADE")
            .execute(&db)
            .await;
        let _ = sqlx::query("DROP TABLE IF EXISTS upload_sessions CASCADE")
            .execute(&db)
            .await;

        let migration_sql =
            std::fs::read_to_string("./migrations/20240104000000_upload_sessions.sql").unwrap();

        // Execute SQL statements split by semicolon
        for statement in migration_sql.split(';') {
            let stmt = statement.trim();
            if !stmt.is_empty() {
                sqlx::query(stmt).execute(&db).await.unwrap();
            }
        }

        let state = AppState {
            db: db.clone(),
            storage: std::sync::Arc::new(DummyStorage),
            master_key: [0u8; 32],
            api_key: "test".to_string(),
        };

        // 1. Create a session
        let req = StartSessionRequest {
            name: "test_session_file.bin".to_string(),
            total_size: 100,
            mime_type: Some("application/octet-stream".to_string()),
            folder_id: None,
            sha256_hash: Some("test_sha".to_string()),
        };

        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", "test".parse().unwrap());

        let res = start_upload_session_handler(State(state.clone()), headers.clone(), Json(req))
            .await
            .unwrap();
        let session_id = res.0.session_id;

        // Verify status
        let row = sqlx::query("SELECT status, name FROM upload_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(row.get::<String, _>("status"), "Created");
        assert_eq!(row.get::<String, _>("name"), "test_session_file.bin");

        // 2. Simulate chunk upload by manually inserting a pending chunk
        let chunk_uuid = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO pending_session_chunks (id, session_id, chunk_number, backend_chunk_id, size_bytes, encrypted_size, chunk_sha256, storage_provider) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
        )
        .bind(chunk_uuid)
        .bind(session_id)
        .bind(0)
        .bind("mock_backend_chunk_id")
        .bind(100i64)
        .bind(116i64)
        // Concatenated hashes of empty sha256 sequence, but we can compute one
        .bind("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        .bind("local")
        .execute(&db)
        .await
        .unwrap();

        // 3. Commit session
        // Since we set sha256_hash = Some("test_sha"), let's update it to the calculated hash to make it match!
        let calculated_sha256 = {
            let mut file_hasher = Sha256::new();
            file_hasher.update(
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".as_bytes(),
            );
            hex::encode(file_hasher.finalize())
        };
        sqlx::query("UPDATE upload_sessions SET sha256_hash = $1 WHERE id = $2")
            .bind(&calculated_sha256)
            .bind(session_id)
            .execute(&db)
            .await
            .unwrap();

        let commit_res =
            commit_upload_session_handler(State(state.clone()), headers.clone(), Path(session_id))
                .await
                .unwrap();
        let file_id = commit_res.0.file_id;

        // Verify file is created with correct name and sha256_hash
        let file_row = sqlx::query("SELECT name, sha256_hash FROM files WHERE id = $1")
            .bind(file_id)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(file_row.get::<String, _>("name"), "test_session_file.bin");
        assert_eq!(
            file_row.get::<Option<String>, _>("sha256_hash"),
            Some(calculated_sha256)
        );

        // Verify chunk moved to final chunks
        let chunk_row = sqlx::query("SELECT backend_chunk_id FROM chunks WHERE file_id = $1")
            .bind(file_id)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(
            chunk_row.get::<String, _>("backend_chunk_id"),
            "mock_backend_chunk_id"
        );

        // Verify session status updated to Completed
        let session_status = sqlx::query("SELECT status FROM upload_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(session_status.get::<String, _>("status"), "Completed");

        // Clean up
        let _ = sqlx::query("DELETE FROM files WHERE id = $1")
            .bind(file_id)
            .execute(&db)
            .await;
        let _ = sqlx::query("DELETE FROM upload_sessions WHERE id = $1")
            .bind(session_id)
            .execute(&db)
            .await;
    }
}
