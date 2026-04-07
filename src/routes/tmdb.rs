use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::{Extension, Json, Router, extract::Path, http::StatusCode, response::IntoResponse, routing::get};
use serde::{Deserialize, Serialize};

use crate::classes::config::TmdbConfig;

// ---------- Inbound TMDB types ----------

#[derive(Deserialize)]
#[serde(tag = "media_type", rename_all = "snake_case")]
enum TrendingItem {
    Movie(TrendingMovie),
    Tv(TrendingTv),
    Person,
}

#[derive(Deserialize)]
struct TrendingMovie {
    id: i64,
    title: String,
    poster_path: Option<String>,
    vote_average: f64,
    #[serde(default)]
    release_date: Option<String>,
}

#[derive(Deserialize)]
struct TrendingTv {
    id: i64,
    name: String,
    poster_path: Option<String>,
    vote_average: f64,
    #[serde(default)]
    first_air_date: Option<String>,
}

#[derive(Deserialize)]
struct TrendingPage {
    results: Vec<TrendingItem>,
    page: u32,
    total_pages: u32,
    total_results: u32,
}

// ---------- Query params ----------

#[derive(Deserialize)]
struct TrendingParams {
    page: Option<u32>,
}

#[derive(Deserialize)]
struct SearchParams {
    q: String,
}

// ---------- Outbound types ----------

#[derive(Clone, Serialize)]
struct PosterItem {
    id: i64,
    media_type: &'static str,
    title: String,
    poster_path: String,
    vote_average: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    year: Option<String>,
}

#[derive(Serialize)]
struct TrendingOut {
    page: u32,
    total_pages: u32,
    total_results: u32,
    results: Vec<PosterItem>,
}

// ---------- Featured cache ----------

const FEATURED_TTL: Duration = Duration::from_secs(600); // 10 minutes

pub struct FeaturedCache {
    inner: Mutex<Option<(Instant, Vec<PosterItem>)>>,
}

impl FeaturedCache {
    pub fn new() -> Self {
        Self { inner: Mutex::new(None) }
    }
}

// ---------- Shared fetch helper ----------

fn extract_year(date: &Option<String>) -> Option<String> {
    date.as_deref()
        .filter(|d| d.len() >= 4)
        .map(|d| d[..4].to_string())
}

fn parse_trending_items(items: Vec<TrendingItem>) -> Vec<PosterItem> {
    items
        .into_iter()
        .filter_map(|item| match item {
            TrendingItem::Movie(m) => m.poster_path.map(|p| PosterItem {
                id: m.id,
                media_type: "movie",
                title: m.title,
                poster_path: p,
                vote_average: m.vote_average,
                year: extract_year(&m.release_date),
            }),
            TrendingItem::Tv(t) => t.poster_path.map(|p| PosterItem {
                id: t.id,
                media_type: "tv",
                title: t.name,
                poster_path: p,
                vote_average: t.vote_average,
                year: extract_year(&t.first_air_date),
            }),
            TrendingItem::Person => None,
        })
        .collect()
}

async fn fetch_trending_page(api_key: &str, page: u32) -> Result<TrendingPage, String> {
    let url = format!(
        "https://api.themoviedb.org/3/trending/all/week?page={page}&api_key={api_key}"
    );
    let resp = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("TMDB request failed: {e}"))?
        .json::<TrendingPage>()
        .await
        .map_err(|e| format!("Failed to parse TMDB response: {e}"))?;
    Ok(resp)
}

// ---------- Handlers ----------

/// Public endpoint for the login page. Returns the first page of trending
/// posters from a server-side cache (TTL: 10 minutes). No authentication
/// required, and the cache naturally limits TMDB API calls to 6/hour.
async fn featured(
    Extension(tmdb): Extension<Arc<TmdbConfig>>,
    Extension(cache): Extension<Arc<FeaturedCache>>,
) -> impl IntoResponse {
    let cached = {
        let guard = cache.inner.lock().unwrap();
        guard
            .as_ref()
            .filter(|(fetched_at, _)| fetched_at.elapsed() < FEATURED_TTL)
            .map(|(_, items)| items.clone())
    };

    if let Some(items) = cached {
        return (StatusCode::OK, Json(items)).into_response();
    }

    let page_data = match fetch_trending_page(&tmdb.api_key, 1).await {
        Ok(data) => data,
        Err(e) => {
            tracing::error!("TMDB featured request failed: {e}");
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": "Failed to reach TMDB" })),
            )
                .into_response();
        }
    };

    let items = parse_trending_items(page_data.results);

    {
        let mut guard = cache.inner.lock().unwrap();
        *guard = Some((Instant::now(), items.clone()));
    }

    (StatusCode::OK, Json(items)).into_response()
}

async fn trending(
    Extension(tmdb): Extension<Arc<TmdbConfig>>,
    axum::extract::Query(params): axum::extract::Query<TrendingParams>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(1).clamp(1, 500);

    let page_data = match fetch_trending_page(&tmdb.api_key, page).await {
        Ok(data) => data,
        Err(e) => {
            tracing::error!("TMDB trending request failed: {e}");
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": "Failed to reach TMDB" })),
            )
                .into_response();
        }
    };

    let out = TrendingOut {
        page: page_data.page,
        total_pages: page_data.total_pages,
        total_results: page_data.total_results,
        results: parse_trending_items(page_data.results),
    };

    (StatusCode::OK, Json(serde_json::to_value(out).unwrap())).into_response()
}

async fn details(
    Extension(tmdb): Extension<Arc<TmdbConfig>>,
    Path((media_type, id)): Path<(String, i64)>,
) -> impl IntoResponse {
    let segment = match media_type.as_str() {
        "movie" => "movie",
        "tv" => "tv",
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "media_type must be 'movie' or 'tv'" })),
            )
                .into_response();
        }
    };

    let append = if segment == "movie" {
        "videos,recommendations,credits,release_dates"
    } else {
        "videos,recommendations,credits,external_ids"
    };

    let url = format!(
        "https://api.themoviedb.org/3/{segment}/{id}?api_key={}&append_to_response={append}",
        tmdb.api_key
    );

    let client = reqwest::Client::new();
    match client.get(&url).send().await {
        Err(e) => {
            tracing::error!("TMDB details request failed: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": "Failed to reach TMDB" })),
            )
                .into_response()
        }
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Err(e) => {
                tracing::error!("Failed to parse TMDB details response: {e}");
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(serde_json::json!({ "error": "Failed to parse TMDB response" })),
                )
                    .into_response()
            }
            Ok(val) => (StatusCode::OK, Json(val)).into_response(),
        },
    }
}

async fn search(
    Extension(tmdb): Extension<Arc<TmdbConfig>>,
    axum::extract::Query(params): axum::extract::Query<SearchParams>,
) -> impl IntoResponse {
    let client = reqwest::Client::new();
    match client
        .get("https://api.themoviedb.org/3/search/multi")
        .query(&[("query", &params.q), ("page", &"1".to_string()), ("api_key", &tmdb.api_key)])
        .send()
        .await
    {
        Err(e) => {
            tracing::error!("TMDB search request failed: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": "Failed to reach TMDB" })),
            )
                .into_response()
        }
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Err(e) => {
                tracing::error!("Failed to parse TMDB search response: {e}");
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(serde_json::json!({ "error": "Failed to parse TMDB response" })),
                )
                    .into_response()
            }
            Ok(val) => (StatusCode::OK, Json(val)).into_response(),
        },
    }
}

// ---------- Top Rated types ----------

#[derive(Deserialize)]
struct TopRatedMovie {
    id: i64,
    title: String,
    poster_path: Option<String>,
    vote_average: f64,
    #[serde(default)]
    release_date: Option<String>,
}

#[derive(Deserialize)]
struct TopRatedTv {
    id: i64,
    name: String,
    poster_path: Option<String>,
    vote_average: f64,
    #[serde(default)]
    first_air_date: Option<String>,
}

#[derive(Deserialize)]
struct TopRatedPage<T> {
    results: Vec<T>,
    #[allow(dead_code)]
    page: u32,
    total_pages: u32,
    total_results: u32,
}

#[derive(Deserialize)]
struct TopRatedParams {
    page: Option<u32>,
    #[serde(rename = "type")]
    media_type: Option<String>,
}

async fn top_rated(
    Extension(tmdb): Extension<Arc<TmdbConfig>>,
    axum::extract::Query(params): axum::extract::Query<TopRatedParams>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(1).clamp(1, 500);
    let media_type = params.media_type.as_deref().unwrap_or("all");
    let client = reqwest::Client::new();

    let mut results: Vec<PosterItem> = Vec::new();
    let mut total_pages: u32 = 0;
    let mut total_results: u32 = 0;

    if media_type == "all" || media_type == "movie" {
        let url = format!(
            "https://api.themoviedb.org/3/movie/top_rated?page={page}&api_key={}",
            tmdb.api_key
        );
        match client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(data) = resp.json::<TopRatedPage<TopRatedMovie>>().await {
                    total_pages = total_pages.max(data.total_pages);
                    total_results += data.total_results;
                    results.extend(data.results.into_iter().filter_map(|m| {
                        m.poster_path.map(|p| PosterItem {
                            id: m.id,
                            media_type: "movie",
                            title: m.title,
                            poster_path: p,
                            vote_average: m.vote_average,
                            year: extract_year(&m.release_date),
                        })
                    }));
                }
            }
            Err(e) => tracing::error!("TMDB top_rated movies request failed: {e}"),
        }
    }

    if media_type == "all" || media_type == "tv" {
        let url = format!(
            "https://api.themoviedb.org/3/tv/top_rated?page={page}&api_key={}",
            tmdb.api_key
        );
        match client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(data) = resp.json::<TopRatedPage<TopRatedTv>>().await {
                    total_pages = total_pages.max(data.total_pages);
                    total_results += data.total_results;
                    results.extend(data.results.into_iter().filter_map(|t| {
                        t.poster_path.map(|p| PosterItem {
                            id: t.id,
                            media_type: "tv",
                            title: t.name,
                            poster_path: p,
                            vote_average: t.vote_average,
                            year: extract_year(&t.first_air_date),
                        })
                    }));
                }
            }
            Err(e) => tracing::error!("TMDB top_rated TV request failed: {e}"),
        }
    }

    // Sort by rating descending when combining both types
    results.sort_by(|a, b| b.vote_average.partial_cmp(&a.vote_average).unwrap_or(std::cmp::Ordering::Equal));

    let out = TrendingOut {
        page,
        total_pages,
        total_results,
        results,
    };

    (StatusCode::OK, Json(serde_json::to_value(out).unwrap())).into_response()
}

// ---------- Discover Feed ----------

#[derive(Serialize, Clone)]
struct DiscoverRow {
    id: &'static str,
    title: &'static str,
    items: Vec<PosterItem>,
}

#[derive(Serialize, Clone)]
struct DiscoverFeed {
    rows: Vec<DiscoverRow>,
}

pub struct DiscoverFeedCache {
    inner: Mutex<Option<(Instant, DiscoverFeed)>>,
}

impl DiscoverFeedCache {
    pub fn new() -> Self {
        Self { inner: Mutex::new(None) }
    }
}

async fn fetch_movie_items(client: &reqwest::Client, url: &str) -> Vec<PosterItem> {
    match client.get(url).send().await {
        Ok(resp) => resp
            .json::<TopRatedPage<TopRatedMovie>>()
            .await
            .map(|data| {
                data.results
                    .into_iter()
                    .filter_map(|m| {
                        m.poster_path.map(|p| PosterItem {
                            id: m.id,
                            media_type: "movie",
                            title: m.title,
                            poster_path: p,
                            vote_average: m.vote_average,
                            year: extract_year(&m.release_date),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Err(e) => {
            tracing::error!("TMDB movie list fetch failed: {e}");
            vec![]
        }
    }
}

async fn fetch_tv_items(client: &reqwest::Client, url: &str) -> Vec<PosterItem> {
    match client.get(url).send().await {
        Ok(resp) => resp
            .json::<TopRatedPage<TopRatedTv>>()
            .await
            .map(|data| {
                data.results
                    .into_iter()
                    .filter_map(|t| {
                        t.poster_path.map(|p| PosterItem {
                            id: t.id,
                            media_type: "tv",
                            title: t.name,
                            poster_path: p,
                            vote_average: t.vote_average,
                            year: extract_year(&t.first_air_date),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Err(e) => {
            tracing::error!("TMDB tv list fetch failed: {e}");
            vec![]
        }
    }
}

async fn discover_feed(
    Extension(tmdb): Extension<Arc<TmdbConfig>>,
    Extension(cache): Extension<Arc<DiscoverFeedCache>>,
) -> impl IntoResponse {
    let cached = {
        let guard = cache.inner.lock().unwrap();
        guard
            .as_ref()
            .filter(|(fetched_at, _)| fetched_at.elapsed() < FEATURED_TTL)
            .map(|(_, feed)| feed.clone())
    };

    if let Some(feed) = cached {
        return (StatusCode::OK, Json(serde_json::to_value(feed).unwrap())).into_response();
    }

    let key = &tmdb.api_key;
    let client = reqwest::Client::new();
    let today = crate::functions::datetime::today_ymd();
    let two_weeks_ago = crate::functions::datetime::days_ago_ymd(14);

    let url_new_releases = format!(
        "https://api.themoviedb.org/3/discover/movie?api_key={key}\
        &sort_by=popularity.desc\
        &with_release_type=4|5\
        &vote_count.gte=10\
        &primary_release_date.lte={today}\
        &primary_release_date.gte={two_weeks_ago}"
    );
    let url_top10 = format!("https://api.themoviedb.org/3/trending/movie/week?api_key={key}");
    let url_popular_tv = format!("https://api.themoviedb.org/3/tv/popular?api_key={key}");
    let url_now_playing = format!("https://api.themoviedb.org/3/movie/now_playing?api_key={key}");
    let url_acclaimed = format!(
        "https://api.themoviedb.org/3/discover/movie?api_key={key}\
        &sort_by=vote_average.desc\
        &vote_count.gte=5000\
        &vote_average.gte=8"
    );
    let url_top_rated_tv = format!("https://api.themoviedb.org/3/tv/top_rated?api_key={key}");
    let url_airing_today = format!("https://api.themoviedb.org/3/tv/airing_today?api_key={key}");

    let (
        trending_res,
        new_releases,
        top10_movies,
        popular_tv,
        now_playing,
        acclaimed,
        top_rated_tv,
        airing_today,
    ) = tokio::join!(
        fetch_trending_page(key, 1),
        fetch_movie_items(&client, &url_new_releases),
        fetch_movie_items(&client, &url_top10),
        fetch_tv_items(&client, &url_popular_tv),
        fetch_movie_items(&client, &url_now_playing),
        fetch_movie_items(&client, &url_acclaimed),
        fetch_tv_items(&client, &url_top_rated_tv),
        fetch_tv_items(&client, &url_airing_today),
    );

    let mut rows = Vec::new();

    if let Ok(data) = trending_res {
        let items = parse_trending_items(data.results);
        if !items.is_empty() {
            rows.push(DiscoverRow { id: "trending", title: "Trending Now", items });
        }
    }

    if !new_releases.is_empty() {
        rows.push(DiscoverRow { id: "new-releases", title: "New Releases", items: new_releases });
    }

    let top10: Vec<PosterItem> = top10_movies.into_iter().take(10).collect();
    if !top10.is_empty() {
        rows.push(DiscoverRow { id: "top-10-movies", title: "Top 10 Movies This Week", items: top10 });
    }

    if !popular_tv.is_empty() {
        rows.push(DiscoverRow { id: "popular-tv", title: "Popular TV Shows", items: popular_tv });
    }

    if !now_playing.is_empty() {
        rows.push(DiscoverRow { id: "now-playing", title: "Now Playing in Theaters", items: now_playing });
    }

    if !acclaimed.is_empty() {
        rows.push(DiscoverRow { id: "acclaimed", title: "Critically Acclaimed", items: acclaimed });
    }

    if !top_rated_tv.is_empty() {
        rows.push(DiscoverRow { id: "top-rated-tv", title: "Top Rated TV", items: top_rated_tv });
    }

    if !airing_today.is_empty() {
        rows.push(DiscoverRow { id: "airing-today", title: "Airing Today", items: airing_today });
    }

    let feed = DiscoverFeed { rows };

    {
        let mut guard = cache.inner.lock().unwrap();
        *guard = Some((Instant::now(), feed.clone()));
    }

    (StatusCode::OK, Json(serde_json::to_value(feed).unwrap())).into_response()
}

// ---------- Find by IMDb ID ----------

async fn find_by_imdb(
    Extension(tmdb): Extension<Arc<TmdbConfig>>,
    Path(imdb_id): Path<String>,
) -> impl IntoResponse {
    let url = format!(
        "https://api.themoviedb.org/3/find/{imdb_id}?external_source=imdb_id&api_key={}",
        tmdb.api_key
    );

    let client = reqwest::Client::new();
    match client.get(&url).send().await {
        Err(e) => {
            tracing::error!("TMDB find request failed: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": "Failed to reach TMDB" })),
            )
                .into_response()
        }
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Err(e) => {
                tracing::error!("Failed to parse TMDB find response: {e}");
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(serde_json::json!({ "error": "Failed to parse TMDB response" })),
                )
                    .into_response()
            }
            Ok(val) => (StatusCode::OK, Json(val)).into_response(),
        },
    }
}

// ---------- Router ----------

pub fn router(cache: Arc<FeaturedCache>, feed_cache: Arc<DiscoverFeedCache>) -> Router<crate::routes::SharedState> {
    Router::new()
        .route("/api/tmdb/featured", get(featured))
        .route("/api/tmdb/trending", get(trending))
        .route("/api/tmdb/top-rated", get(top_rated))
        .route("/api/tmdb/details/{media_type}/{id}", get(details))
        .route("/api/tmdb/find/{imdb_id}", get(find_by_imdb))
        .route("/api/tmdb/search", get(search))
        .route("/api/tmdb/discover-feed", get(discover_feed))
        .layer(Extension(cache))
        .layer(Extension(feed_cache))
}
