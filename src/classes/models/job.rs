use serde::{Deserialize, Serialize};

use crate::classes::config::StageWeightsConfig;
use crate::classes::database::Database;
use crate::classes::models::ts;
use sqlx::{Pool, Postgres};

/// Per-job configuration options, stored as JSON in the `preferences` column.
#[derive(Debug, Clone, Deserialize)]
pub struct JobOptions {
    #[serde(default)]
    pub allow_all_releases: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Pending,
    Indexing,
    Downloading,
    Sterilizing,
    Saving,
    Cleanup,
    Finished,
    Failed,
}

impl Stage {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Stage::Finished | Stage::Failed)
    }

    pub fn to_progress_weight(&self, weights: &StageWeightsConfig) -> f32 {
        match self {
            Stage::Indexing => weights.indexing,
            Stage::Downloading => weights.downloading,
            Stage::Sterilizing => weights.sterilizing,
            Stage::Saving => weights.saving,
            Stage::Cleanup => weights.cleanup,
            _ => 0.0,
        }
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Job {
    pub id: uuid::Uuid,
    pub imdb_id: String,
    pub title: String,
    pub poster_path: Option<String>,
    pub season: Option<i32>,
    pub current_stage: Stage,
    pub last_log: String,
    pub preferences: Option<serde_json::Value>,
    pub progress: serde_json::Value,
    pub user_id: String,
    pub created_at: String,
    pub updated_at: String,
}

impl Job {
    /// Parse job-specific options from the preferences JSON.
    /// Returns defaults if preferences is null or missing fields.
    pub fn options(&self) -> JobOptions {
        self.preferences
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or(JobOptions {
                allow_all_releases: false,
            })
    }

    /// Insert a new job into the database and return it.
    pub async fn create(
        imdb_id: &str,
        title: &str,
        poster_path: Option<&str>,
        season: Option<i32>,
        preferences: Option<serde_json::Value>,
        user_id: &str,
    ) -> Result<Job, sqlx::Error> {
        let pool = Job::pool();

        let job = sqlx::query_as::<_, Job>(concat!(
            "INSERT INTO jobs (imdb_id, title, poster_path, season, preferences, user_id) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             RETURNING id, imdb_id, title, poster_path, season, current_stage, last_log, preferences, progress, user_id, ",
            ts!("created_at"), " as created_at, ",
            ts!("updated_at"), " as updated_at",
        ))
        .bind(imdb_id)
        .bind(title)
        .bind(poster_path)
        .bind(season)
        .bind(preferences)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(job)
    }

    /// Load job from database using ID.
    pub async fn from_id(id: &str) -> Result<Job, sqlx::Error> {
        let pool = Job::pool();

        tracing::debug!("Loading job {id}");

        let uuid = uuid::Uuid::parse_str(id).map_err(|e| {
            tracing::error!("Invalid job ID '{id}': {e}");
            sqlx::Error::Decode(Box::new(e))
        })?;

        let row = sqlx::query_as::<_, Job>(concat!(
            "SELECT id, imdb_id, title, poster_path, season, current_stage, last_log, preferences, progress, user_id, ",
            ts!("created_at"), " as created_at, ",
            ts!("updated_at"), " as updated_at ",
            "FROM jobs WHERE id = $1",
        ))
        .bind(uuid)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to load job {id}: {e}");
            e
        })?;

        tracing::debug!("Loaded job {id}");

        Ok(row)
    }

    /// Save the current job state back to the database, overwriting all fields.
    /// Save the current job state back to the database, overwriting all fields.
    pub async fn save(&self) {
        let pool = Job::pool();

        tracing::debug!("Saving job {}", self.id);

        sqlx::query(
            "UPDATE jobs SET imdb_id = $1, title = $2, poster_path = $3, season = $4, current_stage = $5, last_log = $6, preferences = $7, progress = $8, user_id = $9, updated_at = NOW() WHERE id = $10",
        )
        .bind(&self.imdb_id)
        .bind(&self.title)
        .bind(&self.poster_path)
        .bind(self.season)
        .bind(&self.current_stage)
        .bind(&self.last_log)
        .bind(&self.preferences)
        .bind(&self.progress)
        .bind(&self.user_id)
        .bind(self.id)
        .execute(pool)
        .await
        .expect(&format!("Failed to save job {}", self.id));

        tracing::debug!("Saved job {}", self.id);
    }

    /// Update only the current_stage field in the database.
    pub async fn update_stage(&mut self, stage: Stage) {
        self.current_stage = stage;
        let pool = Job::pool();

        sqlx::query("UPDATE jobs SET current_stage = $1, updated_at = NOW() WHERE id = $2")
            .bind(stage)
            .bind(self.id)
            .execute(pool)
            .await
            .expect(&format!("Failed to update stage for job {}", self.id));
    }

    /// Update only the last_log field in the database.
    pub async fn update_last_log(id: uuid::Uuid, message: &str) {
        let pool = Job::pool();

        sqlx::query("UPDATE jobs SET last_log = $1, updated_at = NOW() WHERE id = $2")
            .bind(message)
            .bind(id)
            .execute(pool)
            .await
            .expect(&format!("Failed to update last_log for job {id}"));
    }

    /// Update only the progress JSONB field in the database.
    pub async fn update_progress(id: uuid::Uuid, progress: &serde_json::Value) {
        let pool = Job::pool();

        sqlx::query("UPDATE jobs SET progress = $1, updated_at = NOW() WHERE id = $2")
            .bind(progress)
            .bind(id)
            .execute(pool)
            .await
            .expect(&format!("Failed to update progress for job {id}"));
    }

    /// Return the first active (non-terminal) job for the given imdb_id, if any.
    pub async fn find_active_by_imdb_id(imdb_id: &str) -> Result<Option<Job>, sqlx::Error> {
        let pool = Job::pool();

        let row = sqlx::query_as::<_, Job>(concat!(
            "SELECT id, imdb_id, title, poster_path, season, current_stage, last_log, preferences, progress, user_id, ",
            ts!("created_at"), " as created_at, ",
            ts!("updated_at"), " as updated_at ",
            "FROM jobs \
             WHERE imdb_id = $1 AND current_stage NOT IN ('finished', 'failed') \
             LIMIT 1",
        ))
        .bind(imdb_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// Fetch jobs using a raw SQL query. The query must return all Job columns.
    /// Timestamp columns must be cast to text via `ts!` (see `models::ts` macro).
    pub async fn from_query(sql: &str) -> Result<Vec<Job>, sqlx::Error> {
        let pool = Job::pool();

        tracing::debug!("Querying jobs: {sql}");

        let rows = sqlx::query_as::<_, Job>(sql)
            .fetch_all(pool)
            .await
            .map_err(|e| {
                tracing::error!("Failed to query jobs: {e}");
                e
            })?;

        tracing::debug!("Query returned {} job(s)", rows.len());

        Ok(rows)
    }
    
    /// List all jobs, paginated and ordered by most recently updated.
    pub async fn list(page: u32, per_page: u32) -> Result<(Vec<Job>, i64), sqlx::Error> {
        let pool = Job::pool();
        let offset = (page.saturating_sub(1)) as i64 * per_page as i64;
        let limit = per_page as i64;

        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM jobs")
            .fetch_one(pool)
            .await?;

        let rows = sqlx::query_as::<_, Job>(concat!(
            "SELECT id, imdb_id, title, poster_path, season, current_stage, last_log, preferences, progress, user_id, ",
            ts!("created_at"), " as created_at, ",
            ts!("updated_at"), " as updated_at ",
            "FROM jobs ORDER BY updated_at DESC LIMIT $1 OFFSET $2",
        ))
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        Ok((rows, total.0))
    }

    /// Delete a job by ID. Only allows deleting jobs in a terminal state.
    pub async fn delete(id: uuid::Uuid) -> Result<bool, sqlx::Error> {
        let pool = Job::pool();

        let result = sqlx::query(
            "DELETE FROM jobs WHERE id = $1 AND current_stage IN ('finished', 'failed')",
        )
        .bind(id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    fn pool() -> &'static Pool<Postgres> {
        &Database::get().pool
    }
}
