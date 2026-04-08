use std::sync::OnceLock;
use sqlx::{Pool, Postgres, postgres::PgPool};
use crate::classes::config::DatabaseConfig;

static INSTANCE: OnceLock<Database> = OnceLock::new();

#[derive(Debug)]
pub struct Database {
    pub pool: Pool<Postgres>,
}

impl Database {
    pub async fn init(config: &DatabaseConfig) {
        tracing::info!("Connecting to database");

        let pool = PgPool::connect(&config.url).await.unwrap_or_else(|e| {
            tracing::error!("Failed to connect to PostgreSQL: {e}");
            eprintln!("Error: Could not connect to PostgreSQL at the configured database URL.\nCheck that PostgreSQL is running and your credentials are correct.");
            std::process::exit(1);
        });

        tracing::info!("Running migrations");

        sqlx::migrate!().run(&pool).await.unwrap_or_else(|e| {
            tracing::error!("Failed to run migrations: {e}");
            eprintln!("Error: Database migration failed. The database may need to be reset or the schema is incompatible.");
            std::process::exit(1);
        });

        tracing::info!("Database ready");

        INSTANCE
            .set(Database { pool })
            .expect("Database already initialized");
    }

    pub fn get() -> &'static Database {
        INSTANCE.get().expect("Database not initialized")
    }
}
