use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Serialize;
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
}

pub async fn list_files_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let files = sqlx::query_as::<_, FileMetadata>(
        r#"
        SELECT id, name, total_size, mime_type, created_at
        FROM files
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database error: {}", e),
        )
    })?;

    Ok(Json(files))
}
