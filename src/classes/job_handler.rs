use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use tokio::sync::RwLock;

use crate::classes::config::{
    JackettConfig, JobsConfig, NamingConfig, PathsConfig, QbittorrentConfig, StageWeightsConfig,
    TmdbConfig,
};
use crate::classes::errors::StageError;
use crate::classes::models::index::Index;
use crate::classes::models::job::{Job, Stage};
use crate::functions::query_jackett::QueryConfig;
use crate::functions::stages::download::{DownloadConfig, download};
use crate::functions::stages::index::{IndexConfig, index};
use crate::functions::stages::sterilize::sterilize;
use crate::functions::stages::save::{SaveConfig, save};
use crate::functions::stages::cleanup::cleanup;

// The configurations that the job handler requires.
#[derive(Clone)]
pub struct HandlerConfig {
    pub paths_config: PathsConfig,
    pub naming_config: NamingConfig,
    pub jobs_config: JobsConfig,
    pub jackett_config: JackettConfig,
    pub tmdb_config: TmdbConfig,
    pub qbittorrent_config: QbittorrentConfig,
}

pub struct JobHandler {
    pub job: RwLock<Job>,
    pub config: HandlerConfig,
}

impl JobHandler {
    pub fn new(job: Job, mut config: HandlerConfig) -> Self {
        let options = job.options();

        if options.allow_all_releases {
            config.jobs_config.scoring.blacklisted_release_types.clear();
        }

        Self {
            job: RwLock::new(job),
            config,
        }
    }

    pub async fn create_logger(&self, stage: Stage) -> JobLogger {
        let job_id = self.job.read().await.id;
        let log_path = self
            .config
            .paths_config
            .logs
            .join(format!("{}.log", job_id));

        let logger = JobLogger {
            job_id,
            log_path,
            stage,
            stage_weights: self.config.jobs_config.stage_weights.clone(),
        };

        logger.log(&format!("Starting {:?}", stage), true).await;
        logger
    }

    pub async fn start(&self) {
        // Index
        let mut index_result = if matches!(
            self.job.read().await.current_stage,
            Stage::Pending | Stage::Indexing
        ) {
            // Complete Index stage: -----------------------------------

            self.job.write().await.update_stage(Stage::Indexing).await;
            let log = self.create_logger(Stage::Indexing).await;

            let query_config = QueryConfig {
                jackett: &self.config.jackett_config,
                tmdb: &self.config.tmdb_config,
            };
            let index_config = IndexConfig {
                query: &query_config,
                scoring: &self.config.jobs_config.scoring,
            };

            match index(index_config, &*self.job.read().await, &log).await {
                Ok(result) => result,
                Err(e) => {
                    log.log(&format!("Indexing failed: {e}"), true).await;

                    self.fail().await;

                    return;
                }
            }
        } else {
            // Skip & pull index from db: --------------------------------

            let job = self.job.read().await;
            match Index::from_imdb(&job.imdb_id, job.season).await {
                Ok(result) => result,
                Err(e) => {
                    tracing::error!("Failed to load index for {}: {e}", job.imdb_id);

                    self.fail().await;

                    return;
                }
            }
        };

        // Download - sterilize retry loop
        let max_retries = self.config.jobs_config.max_retries;
        let (job_imdb_id, job_season) = {
            let job = self.job.read().await;
            (job.imdb_id.clone(), job.season)
        };

        let download_config = DownloadConfig {
            qbittorrent: &self.config.qbittorrent_config,
            download_dir: &self.config.paths_config.download,
            downloading: &self.config.jobs_config.downloading,
        };

        for attempt in 0..max_retries {
            let torrent = match index_result.get_selected_torrent() {
                Some(t) => t,
                None => {
                    let log = self.create_logger(Stage::Downloading).await;
                    log.log("No more torrents available to try", true).await;
                    self.fail().await;
                    return;
                }
            };

            let torrent_id = torrent.id;
            let torrent_title = torrent.title.clone();

            let result: Result<(), StageError> = async {
                self.job.write().await.update_stage(Stage::Downloading).await;
                let log = self.create_logger(Stage::Downloading).await;
                let content_path = download(&download_config, torrent, &log).await?;

                self.job.write().await.update_stage(Stage::Sterilizing).await;
                let log = self.create_logger(Stage::Sterilizing).await;
                let output_dir = sterilize(&log, &self.config.paths_config.download, &content_path, &self.config.jobs_config.media_extensions).await?;

                self.job.write().await.update_stage(Stage::Saving).await;
                let log = self.create_logger(Stage::Saving).await;
                let save_config = SaveConfig {
                    movies_dir: &self.config.paths_config.movies,
                    series_dir: &self.config.paths_config.series,
                    movie_folder: &self.config.naming_config.movie_folder,
                    movie_file: &self.config.naming_config.movie_file,
                    series_folder: &self.config.naming_config.series_folder,
                    season_folder: &self.config.naming_config.season_folder,
                    series_file: &self.config.naming_config.series_file,
                    tmdb_api_key: &self.config.tmdb_config.api_key,
                };
                let _save_dir = save(&log, &save_config, &output_dir, &job_imdb_id, job_season).await?;

                self.job.write().await.update_stage(Stage::Cleanup).await;
                let log = self.create_logger(Stage::Cleanup).await;
                cleanup(&log, &content_path).await?;

                Ok(())
            }
            .await;

            match result {
                Ok(()) => break,
                Err(StageError::Retryable(reason)) => {
                    self.handle_retryable(
                        &mut index_result,
                        torrent_id,
                        &torrent_title,
                        &reason,
                        attempt,
                        max_retries,
                    )
                    .await;

                    if attempt + 1 >= max_retries {
                        self.fail().await;
                        return;
                    }
                }
                Err(StageError::Fatal(reason)) => {
                    self.handle_fatal(&reason).await;
                    return;
                }
            }
        }

        let log = self.create_logger(Stage::Finished).await;
        log.log("Job complete", true).await;
        self.job.write().await.update_stage(Stage::Finished).await;
    }

    /// Handle a retryable error: blacklist the torrent, select the next one, log.
    async fn handle_retryable(
        &self,
        index: &mut Index,
        torrent_id: uuid::Uuid,
        torrent_title: &str,
        reason: &str,
        attempt: u32,
        max_retries: u32,
    ) {
        let log = self.create_logger(Stage::Downloading).await;

        log.log(
            &format!(
                "Attempt {}/{} failed (retryable): {reason}",
                attempt + 1,
                max_retries
            ),
            true,
        )
        .await;

        // Blacklist the failed torrent (in-memory + DB)
        if let Some(torrent) = index.torrents.iter_mut().find(|t| t.id == torrent_id) {
            if let Err(e) = torrent.blacklist(reason).await {
                log.log(&format!("Failed to blacklist torrent in DB: {e}"), false)
                    .await;
            }
        }

        log.log(
            &format!("Blacklisted torrent '{torrent_title}': {reason}"),
            true,
        )
        .await;

        // Select the next best torrent
        match index.select_next_torrent().await {
            Ok(Some(next)) => {
                log.log(&format!("Selected next torrent: {}", next.title), true)
                    .await;
            }
            Ok(None) => {
                log.log("No more non-blacklisted torrents available", true)
                    .await;
            }
            Err(e) => {
                log.log(&format!("Failed to select next torrent: {e}"), false)
                    .await;
            }
        }
    }

    /// Handle a fatal error: log and fail the job.
    async fn handle_fatal(&self, reason: &str) {
        let log = self.create_logger(Stage::Downloading).await;
        log.log(&format!("Fatal error: {reason}"), true).await;
        self.fail().await;
    }

    /// Mark the job as failed.
    async fn fail(&self) {
        self.job.write().await.update_stage(Stage::Failed).await;
    }
}

/// Per-job, per-stage logger created by `JobHandler::create_logger`.
///
/// Owns all the data it needs (job ID, file path, stage, weights) so it can be
/// passed into stage functions without borrowing the handler or the job.
///
/// Two core methods:
/// - `log(message, public)` - Appends a timestamped line to the job's log file.
///   Always emits a tracing debug line. When `public` is true, also updates the
///   job's `last_log` in the database so the frontend can display it.
/// - `progress(percent)` - Takes a 0.0-1.0 value representing stage completion.
///   Updates the job's progress JSONB in the database and logs the weighted
///   overall progress.
pub struct JobLogger {
    job_id: uuid::Uuid,
    log_path: PathBuf,
    stage: Stage,
    stage_weights: StageWeightsConfig,
}

impl JobLogger {
    pub async fn log(&self, message: &str, public: bool) {
        let timestamp = crate::functions::datetime::now_log_timestamp();
        let stage_label = format!("{:?}", self.stage);
        let line = format!("[{timestamp}] [{stage_label}] {message}\n");

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
        {
            let _ = file.write_all(line.as_bytes());
        }

        tracing::debug!("[job:{}] [{stage_label}] {message}", self.job_id);

        if public {
            Job::update_last_log(self.job_id, message).await;
        }
    }

    pub async fn progress(&self, percent: f64) {
        let stage_key = format!("{:?}", self.stage).to_lowercase();
        let stage_value = (percent.clamp(0.0, 1.0) * 100.0).round() as u32;

        let progress = serde_json::json!({ stage_key: stage_value });

        Job::update_progress(self.job_id, &progress).await;

        let weight = self.stage.to_progress_weight(&self.stage_weights) as f64;
        let overall = (percent * weight * 100.0).round();
        self.log(
            &format!("Stage progress: {stage_value}% (overall ~{overall}%)"),
            false,
        )
        .await;
    }
}
