mod classes;
mod functions;
mod routes;

use std::sync::Arc;

use better_auth::{AuthBuilder, AuthConfig, CorsConfig};
use better_auth::adapters::SqlxAdapter;
use better_auth::plugins::{
    AdminPlugin, EmailPasswordPlugin, PasswordManagementPlugin, SessionManagementPlugin,
};
use classes::prerequisite::{Prerequisite, check_binary, check_service, check_tcp, check_vpn};
use classes::{config, logger};

use crate::classes::database::Database;
use crate::classes::job_handler::HandlerConfig;
use crate::classes::job_queue::JobQueue;
use crate::classes::server::{Server};

// --------------- Main --------------- //

#[tokio::main]
async fn main() {
    // Start logger first
    logger::init();
    tracing::info!("Starting up...");

    // Create config handler
    tracing::debug!("Creating config handler");
    let mut config_handler = config::Config::new();

    let config = config_handler.data.take().expect("Config failed to load");

    // -------------- Prerequisites -------------- //

    let qbit_url = format!("{}/api/v2/app/version", config.qbittorrent.url);
    let jackett_url = format!("{}/api/v2.0/server/config", config.jackett.url);

    let prereqs = vec![
        Prerequisite {
            name: "PostgreSQL",
            required: true,
            check: Box::new(|| Box::pin(check_tcp("localhost", 5432))),
            help: "PostgreSQL must be installed and running.\n  \
                   macOS:   brew install postgresql && brew services start postgresql\n  \
                   Linux:   sudo apt install postgresql && sudo systemctl start postgresql\n  \
                   Docker:  docker run -d -p 5432:5432 -e POSTGRES_DB=rusty-findr postgres\n  \
                   Docs:    https://www.postgresql.org/download/",
        },
        Prerequisite {
            name: "qBittorrent",
            required: true,
            check: Box::new(move || Box::pin(check_service(qbit_url.clone()))),
            help: "qBittorrent must be installed with the Web UI enabled.\n  \
                   macOS:   brew install --cask qbittorrent\n  \
                   Linux:   sudo apt install qbittorrent-nox\n  \
                   Docker:  docker run -d -p 8080:8080 linuxserver/qbittorrent\n  \
                   Enable the Web UI in: Preferences > Web UI > Enable\n  \
                   Docs:    https://www.qbittorrent.org/download",
        },
        Prerequisite {
            name: "Jackett",
            required: true,
            check: Box::new(move || Box::pin(check_service(jackett_url.clone()))),
            help: "Jackett must be installed and running.\n  \
                   macOS:   brew install --cask jackett\n  \
                   Linux:   See https://github.com/Jackett/Jackett#installation\n  \
                   Docker:  docker run -d -p 9117:9117 linuxserver/jackett\n  \
                   Docs:    https://github.com/Jackett/Jackett",
        },
        Prerequisite {
            name: "mkvmerge",
            required: true,
            check: Box::new(|| Box::pin(check_binary("mkvmerge"))),
            help: "mkvmerge (part of MKVToolNix) must be installed.\n  \
                   macOS:   brew install mkvtoolnix\n  \
                   Linux:   sudo apt install mkvtoolnix\n  \
                   Docs:    https://mkvtoolnix.download/downloads.html",
        },
        Prerequisite {
            name: "VPN",
            required: false,
            check: Box::new(|| Box::pin(check_vpn())),
            help: "We recommend using a VPN for privacy when downloading torrents.",
        },
    ];

    tracing::info!("Checking prerequisites...");
    let mut failed = false;

    for prereq in &prereqs {
        match (prereq.check)().await {
            Ok(_) => tracing::info!("{} is available", prereq.name),
            Err(e) => {
                if prereq.required {
                    failed = true;
                    tracing::error!("{} is not available: {}", prereq.name, e);
                    tracing::error!("{} setup instructions:\n  {}", prereq.name, prereq.help);
                } else {
                    tracing::warn!("{} is not available: {} {}", prereq.name, e, prereq.help);
                }
            }
        }
    }

    if failed {
        tracing::error!(
            "One or more required prerequisites are not available. Please fix the issues above and restart."
        );
        std::process::exit(1);
    }

    tracing::info!("All prerequisites OK");

    // -------------- Services -------------- //

    Database::init(&config.database).await;

    // TODO: Modify the job queue and handler classes to take config's as reference.
    // Wasting memory and resources by cloning (although cheap).
    let queue_config = HandlerConfig {
        jobs_config: config.jobs.clone(),
        naming_config: config.naming.clone(),
        paths_config: config.paths.clone(),
        jackett_config: config.jackett.clone(),
        tmdb_config: config.tmdb.clone(),
        qbittorrent_config: config.qbittorrent.clone(),
    };
    let job_queue = JobQueue::new(queue_config).await;

    // Create auth service
    let auth_database = SqlxAdapter::new(&config.database.url)
        .await
        .expect("Failed to connect auth database adapter");

    let mut cors = CorsConfig::new()
        .allowed_origin(&config.server.base_url);
    for origin in &config.server.trusted_origins {
        cors = cors.allowed_origin(origin);
    }

    let mut auth_config = AuthConfig::new(&config.auth.secret)
        .base_url(&config.server.base_url);
    for origin in &config.server.trusted_origins {
        auth_config = auth_config.trusted_origin(origin);
    }

    let auth = Arc::new(
        AuthBuilder::new(auth_config)
        .cors(cors)
        .database(auth_database)
        .plugin(EmailPasswordPlugin::new().enable_signup(false))
        .plugin(SessionManagementPlugin::new())
        .plugin(PasswordManagementPlugin::new())
        .plugin(AdminPlugin::new())
        .build()
        .await
        .expect("Failed to initialize auth"),
    );

    // -------------- Server -------------- //

    let port = config.server.port;
    let tmdb_config = Arc::new(config.tmdb.clone());
    let server = Server {
        port,
        auth,
        job_queue: Arc::new(job_queue),
        tmdb_config,
    };

    server.start().await;
}
