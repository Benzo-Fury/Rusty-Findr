use crate::classes::config::ScoringConfig;
use crate::classes::models::torrent::Torrent;

/// Scores a list of torrents based on multiple weighted dimensions.
/// Mutates each torrent's `score` field in place.
pub fn score_torrents(torrents: &mut [Torrent], config: &ScoringConfig) {
    for torrent in torrents.iter_mut() {
        torrent.score = score_single(torrent, config);
    }
}

fn score_single(torrent: &Torrent, config: &ScoringConfig) -> f32 {
    let mut score = 0.0;

    score += score_resolution(torrent, config);
    score += score_file_size(torrent, config);
    score += score_seeders(torrent, config);
    score += score_codec(torrent, config);
    score += score_release_type(torrent, config);
    score += score_release_group(torrent, config);

    // 4K bloat penalty: 2160p files above the configured threshold
    let size_gb = torrent.size_bytes() as f32 / 1_073_741_824.0;
    if torrent.resolution.as_deref() == Some("2160p") && size_gb > config.max_4k_size_gb {
        score -= config.bloat_penalty;
    }

    score
}

/// Scores based on position in the user's preferred resolution list.
/// First entry scores highest, unlisted resolutions score 0.
fn score_resolution(torrent: &Torrent, config: &ScoringConfig) -> f32 {
    let resolution = match &torrent.resolution {
        Some(r) => r,
        None => return 0.0,
    };

    let total = config.resolutions.len();
    if total == 0 {
        return 0.0;
    }

    match config.resolutions.iter().position(|r| r == resolution) {
        Some(idx) => ((total - idx) as f32 / total as f32) * config.resolution_weight,
        None => 0.0,
    }
}

/// Gaussian curve centered on ideal_size_gb with stddev of 4GB.
/// Hard penalties for extremely small or large files.
fn score_file_size(torrent: &Torrent, config: &ScoringConfig) -> f32 {
    let gb = torrent.size_bytes() as f32 / 1_073_741_824.0;

    if gb < 0.5 {
        return -20.0;
    }
    if gb > 30.0 {
        return -15.0;
    }
    if gb > 15.0 {
        return -7.5;
    }

    let deviation = (gb - config.ideal_size_gb) / 4.0;
    (-0.5 * deviation * deviation).exp() * config.file_size_weight
}

/// Logarithmic scaling with no cap. Torrents below min_seeders get a heavy penalty.
fn score_seeders(torrent: &Torrent, config: &ScoringConfig) -> f32 {
    if torrent.seeders < config.min_seeders {
        return -(config.seeders_weight);
    }

    let seeders = torrent.seeders as f32;
    (seeders + 1.0).log10() / 1001_f32.log10() * config.seeders_weight
}

/// Ranks codecs by efficiency: AV1 > H.265 > H.264.
fn score_codec(torrent: &Torrent, config: &ScoringConfig) -> f32 {
    let rank = match torrent.codec.as_deref() {
        Some("AV1") => 3,
        Some("H.265") => 2,
        Some("H.264") => 1,
        _ => 0,
    };

    (rank as f32 / 3.0) * config.codec_weight
}

/// Ranks release types by quality. WEB-DL is best, Remux is worst (huge files
/// that provide diminishing returns after sterilization).
fn score_release_type(torrent: &Torrent, config: &ScoringConfig) -> f32 {
    let rank = match torrent.release_type.as_deref() {
        Some("WEB-DL") => 7,
        Some("WEBRip") => 6,
        Some("BluRay") => 5,
        Some("HDTV") => 4,
        Some("HDRip") => 3,
        Some("BDRip") => 2,
        Some("DVDRip") => 1,
        Some("Remux") => 0,
        _ => 0,
    };

    (rank as f32 / 7.0) * config.release_type_weight
}

/// Flat bonus for torrents from known reputable release groups.
/// Checks the torrent title for group tags (typically at the end after a dash).
fn score_release_group(torrent: &Torrent, config: &ScoringConfig) -> f32 {
    let title_upper = torrent.title.to_uppercase();

    for group in &config.reputable_groups {
        if title_upper.contains(&group.to_uppercase()) {
            return config.release_group_weight;
        }
    }

    0.0
}