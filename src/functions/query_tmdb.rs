use serde::Deserialize;

use crate::classes::config::TmdbConfig;

#[derive(Deserialize)]
struct FindResponse {
    movie_results: Vec<MovieResult>,
    tv_results: Vec<TvResult>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MovieResult {
    pub id: i64,
    pub title: String,
    pub original_title: String,
    pub original_language: String,
    pub overview: String,
    pub release_date: String,
    pub adult: bool,
    pub popularity: f64,
    pub vote_average: f64,
    pub vote_count: i64,
    pub genre_ids: Vec<i64>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub video: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TvResult {
    pub id: i64,
    pub name: String,
    pub original_name: String,
    pub original_language: String,
    pub overview: String,
    pub first_air_date: String,
    pub adult: bool,
    pub popularity: f64,
    pub vote_average: f64,
    pub vote_count: i64,
    pub genre_ids: Vec<i64>,
    pub origin_country: Vec<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
}

pub enum TmdbResult {
    Movie(MovieResult),
    Tv(TvResult),
}

impl TmdbResult {
    pub fn title(&self) -> &str {
        match self {
            TmdbResult::Movie(m) => &m.title,
            TmdbResult::Tv(t) => &t.name,
        }
    }
}

/// Resolves an IMDb ID to movie or TV metadata via TMDB's "Find by external ID" endpoint.
pub async fn query_tmdb(config: &TmdbConfig, imdb_id: &str) -> Result<TmdbResult, String> {
    let url = format!(
        "https://api.themoviedb.org/3/find/{imdb_id}?external_source=imdb_id&api_key={}",
        config.api_key
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("TMDB request failed: {e}"))?
        .json::<FindResponse>()
        .await
        .map_err(|e| format!("Failed to parse TMDB response: {e}"))?;

    if let Some(movie) = response.movie_results.into_iter().next() {
        return Ok(TmdbResult::Movie(movie));
    }

    if let Some(tv) = response.tv_results.into_iter().next() {
        return Ok(TmdbResult::Tv(tv));
    }

    Err(format!("No results found on TMDB for {imdb_id}"))
}
