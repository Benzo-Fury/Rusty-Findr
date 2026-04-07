use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::classes::config::{JackettConfig, TmdbConfig};
use crate::classes::models::torrent::Torrent;
use crate::functions::query_tmdb::query_tmdb;

pub struct QueryConfig<'a> {
    pub jackett: &'a JackettConfig,
    pub tmdb: &'a TmdbConfig,
}

/// Resolves the IMDb ID to a title via TMDB, then queries Jackett via the Torznab API.
/// - Movies: `"{title}"`
/// - Series: `"{title} S{season} complete"`
pub async fn query_jackett(
    config: &QueryConfig<'_>,
    imdb_id: &str,
    season: Option<i32>,
) -> Result<Vec<Torrent>, String> {
    let tmdb_result = query_tmdb(config.tmdb, imdb_id).await?;
    let title = tmdb_result.title();

    let query = match season {
        Some(s) => format!("{title} S{s:02} complete"),
        None => title.to_string(),
    };

    let url = format!(
        "{}/api/v2.0/indexers/all/results/torznab/api",
        config.jackett.url
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let xml = client
        .get(&url)
        .query(&[("apikey", &config.jackett.api_key), ("t", &"search".to_string()), ("q", &query)])
        .send()
        .await
        .map_err(|e| format!("Jackett request failed: {e}"))?
        .text()
        .await
        .map_err(|e| format!("Failed to read Jackett response: {e}"))?;

    parse_torznab_response(&xml)
}

fn parse_torznab_response(xml: &str) -> Result<Vec<Torrent>, String> {
    // Check for Torznab error response (e.g. <error code="100" description="Invalid API Key" />)
    let mut err_reader = Reader::from_str(xml);
    loop {
        match err_reader.read_event() {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) if e.name().as_ref() == b"error" => {
                let mut code = String::new();
                let mut description = String::new();
                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"code" => code = String::from_utf8_lossy(&attr.value).to_string(),
                        b"description" => {
                            description = String::from_utf8_lossy(&attr.value).to_string()
                        }
                        _ => {}
                    }
                }
                return Err(format!("Torznab error {code}: {description}"));
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    let mut reader = Reader::from_str(xml);
    let mut torrents = Vec::new();

    // Per-item state
    let mut in_item = false;
    let mut in_title = false;
    let mut in_size = false;
    let mut in_comments = false;
    let mut title = String::new();
    let mut link = String::new();
    let mut tracker_url = String::new();
    let mut size: u64 = 0;
    let mut seeders: i32 = 0;
    let mut peers: i32 = 0;
    let mut magnet_url: Option<String> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"item" => {
                    in_item = true;
                    title.clear();
                    link.clear();
                    tracker_url.clear();
                    size = 0;
                    seeders = 0;
                    peers = 0;
                    magnet_url = None;
                }
                b"title" if in_item => in_title = true,
                b"size" if in_item => in_size = true,
                b"comments" if in_item => in_comments = true,
                _ => {}
            },
            Ok(Event::Empty(e)) if in_item => {
                let tag = e.name();
                if tag.as_ref() == b"enclosure" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"url" {
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            if link.is_empty() {
                                link = val;
                            }
                        }
                    }
                } else if tag.as_ref() == b"torznab:attr" || tag.as_ref() == b"attr" {
                    let mut attr_name = String::new();
                    let mut attr_value = String::new();

                    for attr in e.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"name" => {
                                attr_name =
                                    String::from_utf8_lossy(&attr.value).to_string();
                            }
                            b"value" => {
                                attr_value =
                                    String::from_utf8_lossy(&attr.value).to_string();
                            }
                            _ => {}
                        }
                    }

                    match attr_name.as_str() {
                        "seeders" => seeders = attr_value.parse().unwrap_or(0),
                        "peers" => peers = attr_value.parse().unwrap_or(0),
                        "size" => size = attr_value.parse().unwrap_or(0),
                        "magneturl" => magnet_url = Some(attr_value),
                        _ => {}
                    }
                }
            }
            Ok(Event::Text(e)) if in_item => {
                let text = reader.decoder().decode(e.as_ref()).unwrap_or_default();
                if in_title {
                    title = text.to_string();
                } else if in_comments {
                    tracker_url = text.to_string();
                } else if in_size && size == 0 {
                    size = text.trim().parse().unwrap_or(0);
                }
            }
            Ok(Event::End(e)) if in_item => match e.name().as_ref() {
                b"title" => in_title = false,
                b"comments" => in_comments = false,
                b"size" => in_size = false,
                b"item" => {
                    in_item = false;
                    let download_link = magnet_url.take().unwrap_or(link.clone());
                    let parsed_tracker_url = if tracker_url.is_empty() {
                        None
                    } else {
                        Some(tracker_url.clone())
                    };
                    if !download_link.is_empty() {
                        torrents.push(Torrent::from_result(
                            title.clone(),
                            download_link,
                            parsed_tracker_url,
                            size,
                            seeders,
                            peers,
                        ));
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Failed to parse Torznab XML: {e}")),
            _ => {}
        }
    }

    Ok(torrents)
}

