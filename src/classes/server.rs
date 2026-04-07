use std::sync::Arc;

use better_auth::BetterAuth;
use better_auth::adapters::SqlxAdapter;
use tokio::net::TcpListener;

use crate::classes::config::TmdbConfig;
use crate::classes::job_queue::JobQueue;
use crate::routes::router;

pub struct Server {
    pub port: u16,
    pub auth: Arc<BetterAuth<SqlxAdapter>>,
    pub job_queue: Arc<JobQueue>,
    pub tmdb_config: Arc<TmdbConfig>,
}

impl Server {
    pub async fn start(&self) {
        let router = router(Arc::clone(&self.auth), Arc::clone(&self.job_queue), Arc::clone(&self.tmdb_config)).with_state(self.auth.clone());

        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port))
            .await
            .unwrap();

        tracing::info!("Server listening on 0.0.0.0:{}", self.port);

        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                tokio::signal::ctrl_c().await.ok();
                tracing::info!("Shutting down");
            })
            .await
            .unwrap();
    }
}
