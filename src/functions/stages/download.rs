use std::path::PathBuf;
use std::time::Duration;

use qbit_rs::Qbit;
use qbit_rs::model::{AddTorrentArg, Credential, GetTorrentListArg, TorrentSource};

use url::Url;

use crate::classes::config::{DownloadingConfig, QbittorrentConfig};
use crate::classes::errors::StageError;
use crate::classes::job_handler::JobLogger;
use crate::classes::models::torrent::Torrent;

pub struct DownloadConfig<'a> {
    pub qbittorrent: &'a QbittorrentConfig,
    pub download_dir: &'a PathBuf,
    pub downloading: &'a DownloadingConfig,
}

pub async fn download(
    config: &DownloadConfig<'_>,
    torrent: &Torrent,
    log: &JobLogger,
) -> Result<PathBuf, StageError> {
    // Create qBittorrent client
    let credentials = Credential::new(
        config.qbittorrent.username.clone(),
        config.qbittorrent.password.clone(),
    );
    let qbit = Qbit::new(&*config.qbittorrent.url, credentials);

    // Convert magnet into Url
    let torrent_src = Url::parse(&torrent.magnet_link)
        .map_err(|e| StageError::Retryable(format!("Invalid magnet link: {e}")))?;
    let download_dir = config
        .download_dir
        .to_str()
        .expect("Download dir should be valid UTF-8")
        .to_string();
    let hash = torrent.get_hash().await?;

    let arg = AddTorrentArg::builder()
        .source(TorrentSource::Urls {
            urls: vec![torrent_src].into(),
        })
        .savepath(download_dir)
        .build();

    // Register with qBittorrent
    qbit.add_torrent(arg).await?;

    // Sleep to allow qbit to register torrent
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Poll until complete
    let poll_interval = config.downloading.poll_interval_secs;
    let min_seeders = config.downloading.min_seeders;
    let min_seeders_timeout = config.downloading.min_seeders_timeout_secs;

    let mut low_seeder_duration_secs: u64 = 0;

    loop {
        // Fetch current torrent state
        let list = qbit
            .get_torrent_list(GetTorrentListArg::builder().hashes(hash.clone()).build())
            .await
            .map_err(|e| StageError::Retryable(format!("Failed to poll torrent: {e}")))?;

        let t = list
            .first()
            .ok_or_else(|| StageError::Retryable("Torrent disappeared from qBittorrent".into()))?;

        let progress = t.progress.unwrap_or(0.0);
        let seeders = t.num_seeds.unwrap_or(0);
        let leechers = t.num_leechs.unwrap_or(0);
        let eta = t.eta.unwrap_or(8640000);

        let eta_str = if eta < 8640000 {
            format!("{}m {}s", eta / 60, eta % 60)
        } else {
            "unknown".to_string()
        };

        // Report progress
        log.log(
            &format!(
                "Progress: {:.1}% | Seeders: {} | Leechers: {} | ETA: {}",
                progress * 100.0,
                seeders,
                leechers,
                eta_str,
            ),
            true,
        )
        .await;
        log.progress(progress).await;

        // Check if complete
        if progress >= 1.0 {
            log.log("Download complete.", true).await;
            let _ = qbit.delete_torrents(vec![hash], false).await;

            let content_path = PathBuf::from(
                t.content_path
                    .clone()
                    .ok_or_else(|| StageError::Fatal("Torrent has no content_path".into()))?,
            );

            // Single-file torrents download as a file, not a folder.
            // Normalize by wrapping in a folder named after the file stem.
            if content_path.is_file() {
                let name = content_path
                    .file_name()
                    .ok_or_else(|| StageError::Fatal("Could not determine file name".into()))?;
                let stem = content_path
                    .file_stem()
                    .ok_or_else(|| StageError::Fatal("Could not determine file stem".into()))?;
                let folder = config.download_dir.join(stem);
                tokio::fs::create_dir_all(&folder)
                    .await
                    .map_err(|e| StageError::Fatal(format!("Failed to create folder: {e}")))?;
                tokio::fs::rename(&content_path, folder.join(name))
                    .await
                    .map_err(|e| StageError::Fatal(format!("Failed to move file: {e}")))?;
                return Ok(folder);
            }

            return Ok(content_path);
        }

        // Seeder watch
        if seeders < min_seeders {
            low_seeder_duration_secs += poll_interval;

            log.log(
                &format!(
                    "Low seeders: {} (below {}) for {}s/{}s",
                    seeders, min_seeders, low_seeder_duration_secs, min_seeders_timeout
                ),
                true,
            )
            .await;

            if low_seeder_duration_secs >= min_seeders_timeout {
                let _ = qbit.delete_torrents(vec![hash.clone()], false).await;
                return Err(StageError::Retryable(format!(
                    "Seeder count stayed below {} for {}s, torrent swarm is unhealthy",
                    min_seeders, min_seeders_timeout
                )));
            }
        } else {
            low_seeder_duration_secs = 0;
        }

        tokio::time::sleep(Duration::from_secs(poll_interval)).await;
    }
}
