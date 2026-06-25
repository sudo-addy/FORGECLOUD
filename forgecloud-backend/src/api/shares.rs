use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::api::download::{stream_file_response, ChunkRecord, FileRecord};
use crate::AppState;

#[derive(Deserialize)]
pub struct CreateShareRequest {
    pub password: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub max_downloads: Option<i32>,
}

#[derive(Serialize, FromRow)]
pub struct ShareResponse {
    pub id: Uuid,
    pub file_id: Uuid,
    pub token: String,
    pub has_password: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub max_downloads: Option<i32>,
    pub download_count: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct PublicShareInfo {
    pub file_name: String,
    pub file_size: i64,
    pub requires_password: bool,
    pub is_expired: bool,
}

#[derive(Deserialize)]
pub struct DownloadShareQuery {
    pub pwd: Option<String>,
}

pub async fn create_share_handler(
    Path(file_id): Path<Uuid>,
    State(state): State<AppState>,
    Json(payload): Json<CreateShareRequest>,
) -> Result<impl IntoResponse, Response> {
    // Generate a 32-char alphanumeric token
    let token: String = Uuid::new_v4().simple().to_string();

    let password_hash = match payload.password {
        Some(pwd) if !pwd.trim().is_empty() => Some(
            hash(pwd, DEFAULT_COST)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())?,
        ),
        _ => None,
    };

    let id = Uuid::new_v4();

    let share = sqlx::query_as::<_, ShareResponse>(
        r#"
        INSERT INTO shares (id, file_id, token, password_hash, expires_at, max_downloads)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, file_id, token, password_hash IS NOT NULL as has_password, expires_at, max_downloads, download_count, created_at
        "#
    )
    .bind(id)
    .bind(file_id)
    .bind(token)
    .bind(password_hash)
    .bind(payload.expires_at)
    .bind(payload.max_downloads)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e)).into_response())?;

    Ok((StatusCode::CREATED, Json(share)))
}

pub async fn list_shares_handler(
    Path(file_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, Response> {
    let shares = sqlx::query_as::<_, ShareResponse>(
        r#"
        SELECT id, file_id, token, password_hash IS NOT NULL as has_password, expires_at, max_downloads, download_count, created_at
        FROM shares
        WHERE file_id = $1
        ORDER BY created_at DESC
        "#
    )
    .bind(file_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e)).into_response())?;

    Ok(Json(shares))
}

pub async fn delete_share_handler(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, Response> {
    let result = sqlx::query("DELETE FROM shares WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database error: {}", e),
            )
                .into_response()
        })?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Share not found").into_response());
    }

    Ok(StatusCode::NO_CONTENT)
}

// Public endpoints

#[derive(FromRow)]
struct InternalShareRecord {
    file_id: Uuid,
    password_hash: Option<String>,
    expires_at: Option<DateTime<Utc>>,
    max_downloads: Option<i32>,
    download_count: i32,
}

pub async fn get_share_info_handler(
    Path(token): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, Response> {
    let share = get_active_share(&token, &state.db)
        .await
        .map_err(|(s, m)| (s, m).into_response())?;

    let file_record = sqlx::query_as::<_, FileRecord>(
        "SELECT name, total_size, mime_type FROM files WHERE id = $1",
    )
    .bind(share.file_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())?;

    let file_record = match file_record {
        Some(f) => f,
        None => return Err((StatusCode::NOT_FOUND, "File not found".to_string()).into_response()),
    };

    let is_expired = if let Some(expires_at) = share.expires_at {
        Utc::now() > expires_at
    } else {
        false
    };

    let is_exhausted = if let Some(max_dl) = share.max_downloads {
        share.download_count >= max_dl
    } else {
        false
    };

    Ok(Json(PublicShareInfo {
        file_name: file_record.name,
        file_size: file_record.total_size,
        requires_password: share.password_hash.is_some(),
        is_expired: is_expired || is_exhausted,
    }))
}

pub async fn download_share_handler(
    Path(token): Path<String>,
    Query(query): Query<DownloadShareQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, axum::response::Response> {
    let share = get_active_share(&token, &state.db)
        .await
        .map_err(|(s, m)| (s, m).into_response())?;

    // Check expiration
    if let Some(expires_at) = share.expires_at {
        if Utc::now() > expires_at {
            return Err((StatusCode::GONE, "Share link has expired").into_response());
        }
    }

    if let Some(max_dl) = share.max_downloads {
        if share.download_count >= max_dl {
            return Err((StatusCode::GONE, "Share link download limit reached").into_response());
        }
    }

    // Check password
    if let Some(hash_str) = share.password_hash {
        let pwd = query.pwd.unwrap_or_default();
        let is_valid = verify(&pwd, &hash_str).unwrap_or(false);
        if !is_valid {
            return Err((StatusCode::UNAUTHORIZED, "Invalid or missing password").into_response());
        }
    }

    // Fetch file and chunks
    let file_record = sqlx::query_as::<_, FileRecord>(
        "SELECT name, total_size, mime_type FROM files WHERE id = $1",
    )
    .bind(share.file_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())?;

    let file_record = match file_record {
        Some(f) => f,
        None => return Err((StatusCode::NOT_FOUND, "File not found").into_response()),
    };

    let chunks = sqlx::query_as::<_, ChunkRecord>(
        "SELECT backend_chunk_id, size_bytes FROM chunks WHERE file_id = $1 ORDER BY chunk_number ASC"
    )
    .bind(share.file_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())?;

    // Increment download count
    let _ = sqlx::query("UPDATE shares SET download_count = download_count + 1 WHERE token = $1")
        .bind(token)
        .execute(&state.db)
        .await;

    stream_file_response(file_record, chunks, state.storage, state.master_key)
}

async fn get_active_share(
    token: &str,
    db: &sqlx::PgPool,
) -> Result<InternalShareRecord, (StatusCode, String)> {
    let share = sqlx::query_as::<_, InternalShareRecord>(
        r#"
        SELECT file_id, password_hash, expires_at, max_downloads, download_count
        FROM shares
        WHERE token = $1
        "#,
    )
    .bind(token)
    .fetch_optional(db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database error: {}", e),
        )
    })?;

    match share {
        Some(s) => Ok(s),
        None => Err((StatusCode::NOT_FOUND, "Share link not found".to_string())),
    }
}
