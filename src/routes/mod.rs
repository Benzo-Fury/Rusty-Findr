mod health;
mod indexes;
mod jobs;
mod tmdb;
mod web;

use std::sync::Arc;

use axum::{Extension, Router, response::Redirect, routing::get};
use better_auth::{AxumIntegration, BetterAuth};
use better_auth::adapters::SqlxAdapter;

use crate::classes::config::TmdbConfig;
use crate::classes::job_queue::JobQueue;
use crate::routes::tmdb::{FeaturedCache, DiscoverFeedCache};

pub type SharedState = Arc<BetterAuth<SqlxAdapter>>;

pub fn router(auth: SharedState, job_queue: Arc<JobQueue>, tmdb_config: Arc<TmdbConfig>) -> Router<SharedState> {
    let auth_router = auth.clone().axum_router().with_state(auth);
    let featured_cache = Arc::new(FeaturedCache::new());
    let discover_feed_cache = Arc::new(DiscoverFeedCache::new());

    let router = Router::new()
        .route("/", get(|| async { Redirect::permanent("/web/") }))
        .merge(health::router())
        .merge(indexes::router())
        .merge(jobs::router())
        .merge(tmdb::router(featured_cache, discover_feed_cache))
        .nest("/api/auth", auth_router)
        .layer(Extension(job_queue))
        .layer(Extension(tmdb_config));

    // Dev: proxy to Vite dev server
    #[cfg(debug_assertions)]
    let router = router
        .route("/web", get(web::vite_proxy))
        .route("/web/{*path}", get(web::vite_proxy));

    // Prod: serve embedded static files
    #[cfg(not(debug_assertions))]
    let router = router
        .route("/web", get(web::index_handler))
        .route("/web/", get(web::index_handler))
        .route("/web/{*path}", get(web::static_handler));

    router
}
