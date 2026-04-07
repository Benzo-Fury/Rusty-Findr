use std::sync::Arc;

use axum::{Extension, Json, Router, extract::Path, http::StatusCode, response::IntoResponse, routing::{delete, get, post}};
use better_auth::{CurrentSession, adapters::SqlxAdapter};
use serde::Deserialize;

use crate::classes::job_queue::JobQueue;
use crate::classes::models::job::Job;

#[derive(Deserialize)]
struct CreateJobRequest {
    imdb_id: String,
    title: String,
    poster_path: Option<String>,
    season: Option<i32>,
    preferences: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct ListParams {
    page: Option<u32>,
    per_page: Option<u32>,
}

pub fn router() -> Router<crate::routes::SharedState> {
    Router::new()
        .route("/api/jobs", get(list_jobs).post(create_job))
        .route("/api/jobs/{id}", delete(delete_job))
}

async fn create_job(
    Extension(job_queue): Extension<Arc<JobQueue>>,
    session: CurrentSession<SqlxAdapter>,
    Json(body): Json<CreateJobRequest>,
) -> impl IntoResponse {
    let user_id = &session.user.id;

    match Job::find_active_by_imdb_id(&body.imdb_id).await {
        Ok(Some(existing)) => {
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "An active job already exists for this IMDb ID",
                    "job_id": existing.id,
                })),
            );
        }
        Err(e) => {
            tracing::error!("Failed to check for active job: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Failed to check for existing job" })),
            );
        }
        Ok(None) => {}
    }

    let job = match Job::create(&body.imdb_id, &body.title, body.poster_path.as_deref(), body.season, body.preferences, user_id).await {
        Ok(job) => job,
        Err(e) => {
            tracing::error!("Failed to create job: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Failed to create job" })),
            );
        }
    };

    let response = serde_json::json!({
        "id": job.id,
        "imdb_id": job.imdb_id,
        "season": job.season,
        "current_stage": job.current_stage,
    });

    tracing::info!("Created job {} for {}", job.id, job.imdb_id);

    job_queue.push(job).await;

    (StatusCode::CREATED, Json(response))
}

async fn list_jobs(
    axum::extract::Query(params): axum::extract::Query<ListParams>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).clamp(1, 100);

    match Job::list(page, per_page).await {
        Ok((jobs, total)) => {
            let total_pages = (total as f64 / per_page as f64).ceil() as u32;
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "page": page,
                    "per_page": per_page,
                    "total": total,
                    "total_pages": total_pages,
                    "results": jobs,
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list jobs: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Failed to fetch jobs" })),
            )
                .into_response()
        }
    }
}

async fn delete_job(Path(id): Path<String>) -> impl IntoResponse {
    let uuid = match uuid::Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "Invalid job ID" })),
            )
                .into_response();
        }
    };

    match Job::delete(uuid).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Job not found or not in a terminal state" })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to delete job {id}: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Failed to delete job" })),
            )
                .into_response()
        }
    }
}
