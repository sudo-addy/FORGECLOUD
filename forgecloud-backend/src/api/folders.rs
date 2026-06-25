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
    // 1. Begin transaction to safely retrieve structure and perform DB delete
    let mut tx = state.db.begin().await.map_err(|e| {
        err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to start transaction: {}", e),
        )
    })?;

    // 2. Fetch all nested folder IDs recursively (including parent)
    let folder_ids = sqlx::query_scalar::<_, Uuid>(
        "WITH RECURSIVE folder_tree AS ( \
            SELECT id FROM folders WHERE id = $1 \
            UNION ALL \
            SELECT f.id FROM folders f \
            JOIN folder_tree ft ON f.parent_id = ft.id \
         ) \
         SELECT id FROM folder_tree"
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| {
        err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to retrieve folder structure: {}", e),
        )
    })?;

    // 3. Fetch all files inside those folders
    let file_ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM files WHERE folder_id = ANY($1)"
    )
    .bind(&folder_ids)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| {
        err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to retrieve folder files: {}", e),
        )
    })?;

    // 4. Fetch all chunks for those files
    let chunk_ids = if file_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_scalar::<_, String>(
            "SELECT backend_chunk_id FROM chunks WHERE file_id = ANY($1)"
        )
        .bind(&file_ids)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| {
            err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to retrieve file chunks: {}", e),
            )
        })?
    };

    // 5. Delete physical chunks from storage (non-blocking best-effort deletion)
    for cid in chunk_ids {
        if let Err(e) = state.storage.delete_chunk(&cid).await {
            tracing::warn!("Failed to delete physical chunk {} from storage: {}", cid, e);
        }
    }

    // 6. Delete folder from database (cascades files & chunks via DB constraints)
    sqlx::query("DELETE FROM folders WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to delete folder record: {}", e),
            )
        })?;

    // 7. Commit the transaction
    tx.commit().await.map_err(|e| {
        err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to commit folder deletion: {}", e),
        )
    })?;

    Ok(Json(FolderResponse {
        message: "Folder deleted recursively".into(),
        id: Some(id),
    }))
}
