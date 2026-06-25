use axum::extract::{Path, Query};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::AppState;

#[derive(Serialize, FromRow)]
pub struct FileMetadata {
    pub id: Uuid,
    pub name: String,
    pub total_size: i64,
    pub mime_type: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub folder_id: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub folder_id: Option<Uuid>,
}

pub async fn list_files_handler(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let files = if let Some(fid) = query.folder_id {
        sqlx::query_as::<_, FileMetadata>(
            r#"
            SELECT id, name, total_size, mime_type, created_at, folder_id
            FROM files
            WHERE folder_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(fid)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, FileMetadata>(
            r#"
            SELECT id, name, total_size, mime_type, created_at, folder_id
            FROM files
            WHERE folder_id IS NULL
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&state.db)
        .await
    }
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database error: {}", e),
        )
    })?;

    Ok(Json(files))
}

#[derive(Deserialize)]
pub struct UpdateFileRequest {
    pub folder_id: Option<Uuid>,
}

#[derive(Serialize)]
pub struct UpdateFileResponse {
    pub message: String,
}

pub async fn update_file_handler(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<UpdateFileRequest>,
) -> Result<axum::Json<UpdateFileResponse>, (StatusCode, String)> {
    sqlx::query("UPDATE files SET folder_id = $1 WHERE id = $2")
        .bind(payload.folder_id)
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?;

    Ok(axum::Json(UpdateFileResponse {
        message: "File moved".into(),
    }))
}
