use std::path::{Path, PathBuf};

use tokio::fs;

use crate::classes::config::TmdbConfig;
use crate::classes::errors::StageError;
use crate::classes::job_handler::JobLogger;
use crate::functions::query_tmdb::{TmdbResult, query_tmdb};
use crate::functions::walk_files::walk_files;

pub struct SaveConfig<'a> {
    pub movies_dir: &'a Path,
    pub series_dir: &'a Path,
    pub movie_folder: &'a str,
    pub movie_file: &'a str,
    pub series_folder: &'a str,
    pub season_folder: &'a str,
    pub series_file: &'a str,
    pub tmdb_api_key: &'a str,
}

/// Replace `{key}` placeholders in `template` with values from `vars`.
fn apply_template(template: &str, vars: &[(&str, &str)]) -> String {
    let mut result = template.to_owned();
    for (key, value) in vars {
        result = result.replace(&format!("{{{key}}}"), value);
    }
    result
}

/// Parse episode number from a file stem containing SxxExx (e.g. "Show.S01E05").
/// Returns None if no match is found.
fn parse_episode(stem: &str) -> Option<u32> {
    let bytes = stem.as_bytes();
    for i in 0..bytes.len().saturating_sub(3) {
        if bytes[i].to_ascii_uppercase() == b'E' && bytes[i + 1].is_ascii_digit() {
            // Ensure it's preceded by a digit (part of SxxExx, not a word boundary 'E')
            if i > 0 && bytes[i - 1].is_ascii_digit() {
                let digits: String = stem[i + 1..]
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if !digits.is_empty() {
                    return digits.parse().ok();
                }
            }
        }
    }
    None
}

/// Move sterilized files into the Plex library using naming templates.
/// Returns the destination folder path.
pub async fn save(
    log: &JobLogger,
    config: &SaveConfig<'_>,
    output_dir: &PathBuf,
    imdb_id: &str,
    season: Option<i32>,
) -> Result<PathBuf, StageError> {

    // Collect .mkv files from the output directory
    let all_files = walk_files(output_dir).await
        .map_err(|e| StageError::Fatal(format!("Failed to scan output dir: {e}")))?;
    let files: Vec<PathBuf> = all_files
        .into_iter()
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("mkv"))
        .collect();

    if files.is_empty() {
        return Err(StageError::Fatal("No .mkv files found in output directory".into()));
    }

    // Query TMDB for authoritative title and year
    log.log("Fetching metadata from TMDB", true).await;
    let tmdb = query_tmdb(&TmdbConfig { api_key: config.tmdb_api_key.to_owned() }, imdb_id)
        .await
        .map_err(|e| StageError::Fatal(format!("TMDB lookup failed: {e}")))?;

    let (title, year) = match &tmdb {
        TmdbResult::Movie(m) => (m.title.as_str(), &m.release_date[..4]),
        TmdbResult::Tv(t) => (t.name.as_str(), &t.first_air_date[..4]),
    };

    let total = files.len() as f64;

    match season {
        // Movie: one file goes to {movies}/{movie_folder}/{movie_file}.mkv
        None => {
            let folder_name = apply_template(config.movie_folder, &[("title", title), ("year", year)]);
            let file_name = apply_template(config.movie_file, &[("title", title), ("year", year)]);

            let dest_folder = config.movies_dir.join(&folder_name);
            fs::create_dir_all(&dest_folder)
                .await
                .map_err(|e| StageError::Fatal(format!("Failed to create movie folder: {e}")))?;

            let dest_file = dest_folder.join(format!("{file_name}.mkv"));

            // Overwrite existing file if present
            if fs::try_exists(&dest_file).await.unwrap_or(false) {
                fs::remove_file(&dest_file).await
                    .map_err(|e| StageError::Fatal(format!("Failed to remove existing file: {e}")))?;
            }

            let src = files.into_iter().next().unwrap();
            log.log(&format!("Saving: {}", dest_file.display()), true).await;
            fs::rename(&src, &dest_file)
                .await
                .map_err(|e| StageError::Fatal(format!("Failed to move file: {e}")))?;

            log.progress(1.0).await;

            Ok(dest_folder)
        }

        // Series: each episode goes to {series}/{series_folder}/{season_folder}/{series_file}.mkv
        Some(season_num) => {
            let folder_name = apply_template(config.series_folder, &[("title", title), ("year", year)]);
            let season_str = season_num.to_string();
            let season_padded = format!("{season_num:02}");
            let season_folder_name = apply_template(config.season_folder, &[("season", &season_str)]);

            let dest_folder = config.series_dir.join(&folder_name).join(&season_folder_name);
            fs::create_dir_all(&dest_folder)
                .await
                .map_err(|e| StageError::Fatal(format!("Failed to create series folder: {e}")))?;

            for (i, src) in files.iter().enumerate() {
                let stem = src.file_stem().unwrap_or_default().to_string_lossy();
                let episode_num = parse_episode(&stem)
                    .ok_or_else(|| StageError::Fatal(format!("Could not parse episode number from: {stem}")))?;
                let episode_padded = format!("{episode_num:02}");

                let file_name = apply_template(config.series_file, &[
                    ("title", title),
                    ("season", &season_padded),
                    ("episode", &episode_padded),
                ]);

                let dest_file = dest_folder.join(format!("{file_name}.mkv"));

                if fs::try_exists(&dest_file).await.unwrap_or(false) {
                    fs::remove_file(&dest_file).await
                        .map_err(|e| StageError::Fatal(format!("Failed to remove existing file: {e}")))?;
                }

                log.log(&format!("Saving: {}", dest_file.display()), true).await;
                fs::rename(src, &dest_file)
                    .await
                    .map_err(|e| StageError::Fatal(format!("Failed to move file: {e}")))?;

                log.progress((i + 1) as f64 / total).await;
            }

            Ok(config.series_dir.join(&folder_name))
        }
    }
}
