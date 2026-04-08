use axum::{Json, Router, extract::Path, http::StatusCode, response::IntoResponse, routing::{delete, get}};
use serde::Deserialize;

use crate::classes::models::index::Index;

#[derive(Deserialize)]
struct ListParams {
    page: Option<u32>,
    per_page: Option<u32>,
}

async fn list_indexes(
    axum::extract::Query(params): axum::extract::Query<ListParams>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).clamp(1, 100);

    match Index::list(page, per_page).await {
        Ok((indexes, total)) => {
            let total_pages = (total as f64 / per_page as f64).ceil() as u32;
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "page": page,
                    "per_page": per_page,
                    "total": total,
                    "total_pages": total_pages,
                    "results": indexes,
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list indexes: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Failed to fetch indexes" })),
            )
                .into_response()
        }
    }
}

async fn lookup_by_imdb(Path(imdb_id): Path<String>) -> impl IntoResponse {
    match Index::find_all_by_imdb(&imdb_id).await {
        Ok(indexes) => (StatusCode::OK, Json(serde_json::json!({ "indexes": indexes }))).into_response(),
        Err(e) => {
            tracing::error!("Failed to lookup indexes for {imdb_id}: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Failed to lookup indexes" })),
            )
                .into_response()
        }
    }
}

async fn delete_index(Path(id): Path<String>) -> impl IntoResponse {
    let uuid = match uuid::Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "Invalid index ID" })),
            )
                .into_response();
        }
    };

    match Index::delete(uuid).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Index not found" })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to delete index {id}: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Failed to delete index" })),
            )
                .into_response()
        }
    }
}

pub fn router() -> Router<crate::routes::SharedState> {
    Router::new()
        .route("/api/indexes", get(list_indexes))
        .route("/api/indexes/lookup/{imdb_id}", get(lookup_by_imdb))
        .route("/api/indexes/{id}", delete(delete_index))
}
