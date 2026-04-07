#[derive(serde::Serialize, sqlx::FromRow)]
pub struct Torrent {
    pub id: uuid::Uuid,
    pub index_id: uuid::Uuid,
    pub title: String,
    pub magnet_link: String,
    pub size_mb: i32,
    pub seeders: i32,
    pub leechers: i32,
    pub resolution: Option<String>,
    pub codec: Option<String>,
    pub release_type: Option<String>,
    pub tracker_url: Option<String>,
    pub score: f32,
    pub blacklisted: bool,
    pub blacklisted_reason: Option<String>,
    pub created_at: String,
}

impl Torrent {
    /// Build a Torrent from raw search result fields, parsing resolution/codec/release type
    /// from the title.
    pub fn from_result(
        title: String,
        magnet_link: String,
        tracker_url: Option<String>,
        size_bytes: u64,
        seeders: i32,
        peers: i32,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            index_id: uuid::Uuid::nil(),
            resolution: parse_resolution(&title),
            codec: parse_codec(&title),
            release_type: parse_release_type(&title),
            title,
            magnet_link,
            tracker_url,
            size_mb: (size_bytes / 1_000_000) as i32,
            seeders,
            leechers: peers.saturating_sub(seeders),
            score: 0.0,
            blacklisted: false,
            blacklisted_reason: None,
            created_at: String::new(),
        }
    }

    /// Extract the info hash from the magnet link.
    /// If the link is a Jackett proxy URL, resolves it first.
    pub async fn get_hash(&self) -> Result<String, Box<dyn std::error::Error>> {
        let magnet = resolve_magnet(&self.magnet_link).await?;

        let prefix = "xt=urn:btih:";
        let start = magnet.find(prefix)
            .ok_or("No info hash found in magnet link")?
            + prefix.len();
        let rest = &magnet[start..];
        let end = rest.find('&').unwrap_or(rest.len());
        Ok(rest[..end].to_string())
    }

    /// Size in bytes, derived from the stored MB value.
    pub fn size_bytes(&self) -> u64 {
        self.size_mb as u64 * 1_000_000
    }

    /// Mark this torrent as blacklisted in memory and in the database.
    pub async fn blacklist(&mut self, reason: &str) -> Result<(), sqlx::Error> {
        self.blacklisted = true;
        self.blacklisted_reason = Some(reason.to_string());

        let pool = &crate::classes::database::Database::get().pool;
        sqlx::query("UPDATE torrents SET blacklisted = true, blacklisted_reason = $1 WHERE id = $2")
            .bind(reason)
            .bind(self.id)
            .execute(pool)
            .await?;

        Ok(())
    }
}

fn parse_resolution(title: &str) -> Option<String> {
    let lower = title.to_lowercase();
    for res in ["2160p", "1080p", "720p", "480p"] {
        if lower.contains(res) {
            return Some(res.to_string());
        }
    }
    None
}

fn parse_codec(title: &str) -> Option<String> {
    let lower = title.to_lowercase();
    for (pattern, name) in [
        ("av1", "AV1"),
        ("h.265", "H.265"),
        ("x265", "H.265"),
        ("hevc", "H.265"),
        ("h.264", "H.264"),
        ("x264", "H.264"),
        ("avc", "H.264"),
    ] {
        if lower.contains(pattern) {
            return Some(name.to_string());
        }
    }
    None
}

fn parse_release_type(title: &str) -> Option<String> {
    let lower = title.to_lowercase();
    for (pattern, name) in [
        ("remux", "Remux"),
        ("bluray", "BluRay"),
        ("blu-ray", "BluRay"),
        ("web-dl", "WEB-DL"),
        ("webdl", "WEB-DL"),
        ("webrip", "WEBRip"),
        ("web-rip", "WEBRip"),
        ("hdtv", "HDTV"),
        ("dvdrip", "DVDRip"),
        ("bdrip", "BDRip"),
        ("cam", "CAM"),
        ("ts", "TS"),
    ] {
        if lower.contains(pattern) {
            return Some(name.to_string());
        }
    }
    None
}

async fn resolve_magnet(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    if url.starts_with("magnet:") {
        return Ok(url.to_string());
    }

    // Jackett proxy URL -- disable redirects since magnet: isn't HTTP
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let response = client.get(url).send().await?;

    let location = response
        .headers()
        .get("Location")
        .and_then(|v| v.to_str().ok())
        .ok_or("Jackett URL did not redirect to a magnet link")?
        .to_string();

    if location.starts_with("magnet:") {
        Ok(location)
    } else {
        Err("Resolved URL is not a magnet link".into())
    }
}
