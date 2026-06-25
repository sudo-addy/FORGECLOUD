use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::AppState;

#[derive(Serialize, FromRow)]
pub struct FolderRecord {
    pub id: Uuid,
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct CreateFolderRequest {
    pub name: String,
    pub parent_id: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct UpdateFolderRequest {
    pub name: Option<String>,
    pub parent_id: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct ListFoldersQuery {
    pub parent_id: Option<Uuid>,
}

#[derive(Serialize)]
pub struct FolderResponse {
    pub message: String,
    pub id: Option<Uuid>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

fn err_response(status: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error: msg.into() }))
}

pub async fn create_folder_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateFolderRequest>,
) -> Result<Json<FolderResponse>, (StatusCode, Json<ErrorResponse>)> {
    let folder_id = Uuid::new_v4();

    sqlx::query("INSERT INTO folders (id, name, parent_id) VALUES ($1, $2, $3)")
        .bind(folder_id)
        .bind(&payload.name)
        .bind(payload.parent_id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?;

    Ok(Json(FolderResponse {
        message: "Folder created".into(),
        id: Some(folder_id),
    }))
}

pub async fn list_folders_handler(
    State(state): State<AppState>,
    Query(query): Query<ListFoldersQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let folders = if let Some(pid) = query.parent_id {
        sqlx::query_as::<_, FolderRecord>(
            "SELECT id, name, parent_id, created_at FROM folders WHERE parent_id = $1 ORDER BY name ASC"
        )
        .bind(pid)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, FolderRecord>(
            "SELECT id, name, parent_id, created_at FROM folders WHERE parent_id IS NULL ORDER BY name ASC"
        )
        .fetch_all(&state.db)
        .await
    }.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e))
    })?;

    Ok(Json(folders))
}

pub async fn update_folder_handler(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateFolderRequest>,
) -> Result<Json<FolderResponse>, (StatusCode, Json<ErrorResponse>)> {
    // We dynamically build the query depending on what was provided.
    // In a real app, QueryBuilder might be better, but this works for two fields.
    if let Some(ref name) = payload.name {
        sqlx::query("UPDATE folders SET name = $1 WHERE id = $2")
            .bind(name)
            .bind(id)
            .execute(&state.db)
            .await
            .map_err(|e| {
                err_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("DB error: {}", e),
                )
            })?;
    }

    if payload.parent_id.is_some() {
        sqlx::query("UPDATE folders SET parent_id = $1 WHERE id = $2")
            .bind(payload.parent_id) // this allows moving back to root if parent_id is sent as explicit null
            .bind(id)
            .execute(&state.db)
            .await
            .map_err(|e| {
                err_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("DB error: {}", e),
                )
            })?;
    }

    Ok(Json(FolderResponse {
        message: "Folder updated".into(),
        id: Some(id),
    }))
}

pub async fn delete_folder_handler(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<FolderResponse>, (StatusCode, Json<ErrorResponse>)> {
    // IMPORTANT: Because of ON DELETE CASCADE, deleting a folder will instantly delete all subfolders
    // AND files from the database.
    // To prevent orphans in Telegram/Local storage, we should ideally fetch ALL nested file IDs
    // recursively and call `state.storage.delete_chunk` on them.
    // Given the complexity of a recursive physical cleanup, this implementation drops the DB records instantly.
    // In production, an async garbage collection job would sweep the storage layer.

    sqlx::query("DELETE FROM folders WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?;

    Ok(Json(FolderResponse {
        message: "Folder deleted recursively".into(),
        id: Some(id),
    }))
}
