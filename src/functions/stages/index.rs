use crate::classes::config::ScoringConfig;
use crate::classes::job_handler::JobLogger;
use crate::classes::models::index::Index;
use crate::classes::models::job::Job;
use crate::classes::models::torrent::Torrent;
use crate::functions::query_jackett::{QueryConfig, query_jackett};
use crate::functions::score_torrents::score_torrents;

pub struct IndexConfig<'a> {
    pub query: &'a QueryConfig<'a>,
    pub scoring: &'a ScoringConfig,
}

/// Query & Decide are combined together to create "index".
/// Queries Jackett, scores results, deletes any existing index, and persists the new one.
pub async fn index(
    config: IndexConfig<'_>,
    job: &Job,
    logger: &JobLogger,
) -> Result<Index, String> {
    logger.log("Querying Jackett for torrents...", true).await;
    let torrents = query_jackett(config.query, &job.imdb_id, job.season).await?;

    logger
        .log(
            &format!(
                "Found {} torrent(s), filtering blacklisted items.",
                torrents.len()
            ),
            false,
        )
        .await;
    logger.progress(0.3).await;

    // Filter out blacklisted release types.
    let blacklist = config.scoring.blacklisted_release_types();
    let torrents: Vec<Torrent> = torrents
        .into_iter()
        .filter(|t| {
            t.release_type
                .as_ref()
                .map(|rt| !blacklist.contains(&rt.to_uppercase()))
                .unwrap_or(true)
        })
        .collect();

    // Filter out individual episodes - only keep full season packs for series
    let torrents = if job.season.is_some() {
        torrents
            .into_iter()
            .filter(|t| !has_episode_marker(&t.title))
            .collect()
    } else {
        torrents
    };

    if torrents.is_empty() {
        // Handler will log this
        return Err("No suitable torrents found".to_string());
    }

    logger
        .log(
            &format!("{} torrent(s) remaining after filtering", torrents.len()),
            false,
        )
        .await;
    logger.progress(0.5).await;

    // Score all torrents based on weights and scoring algorithm
    let mut torrents = torrents;
    score_torrents(&mut torrents, &config.scoring);

    // Keep only the top 5 highest scored torrents
    torrents.sort_by(|a, b| b.score.total_cmp(&a.score));
    torrents.truncate(5);

    logger.log("Scored and ranked torrents", false).await;

    logger.progress(0.7).await;

    let selected = torrents.first().map(|t| t.id);
    let torrent_count = torrents.len();
    let index = Index::new(&job.imdb_id, job.season, &job.user_id, torrents, selected);

    index
        .save()
        .await
        .map_err(|e| format!("Failed to save index: {e}"))?;

    logger
        .log(
            &format!("Index saved with {} torrent(s)", torrent_count),
            true,
        )
        .await;
    logger.progress(1.0).await;

    Ok(index)
}

/// Checks if a torrent title contains an episode marker (e.g. E01, E5).
fn has_episode_marker(title: &str) -> bool {
    let upper = title.to_uppercase();
    upper
        .find('E')
        .map(|i| upper.as_bytes().get(i + 1).is_some_and(|b| b.is_ascii_digit()))
        .unwrap_or(false)
}