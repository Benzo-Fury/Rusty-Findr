use axum::{Router, routing::get};

use crate::routes::SharedState;

pub fn router() -> Router<SharedState> {
    Router::new().route("/api/health", get(health))
}

async fn health() -> &'static str {
    "ok"
}
