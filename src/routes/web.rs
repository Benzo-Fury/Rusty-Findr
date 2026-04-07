#[cfg(debug_assertions)]
mod dev {
    use axum::{
        extract::Request,
        http::{StatusCode, Uri},
        response::{IntoResponse, Redirect},
    };

    const VITE_BASE: &str = "http://localhost:5173";

    pub async fn vite_proxy(req: Request) -> impl IntoResponse {
        let path_and_query = req
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");

        match format!("{VITE_BASE}{path_and_query}").parse::<Uri>() {
            Ok(uri) => Redirect::temporary(uri.to_string().as_str()).into_response(),
            Err(_) => StatusCode::BAD_GATEWAY.into_response(),
        }
    }
}

#[cfg(debug_assertions)]
pub use dev::vite_proxy;

#[cfg(not(debug_assertions))]
mod prod {
    use axum::{
        extract::Path,
        http::{StatusCode, header},
        response::{Html, IntoResponse, Response},
    };
    use rust_embed::Embed;

    #[derive(Embed)]
    #[folder = "web/dist/"]
    struct WebAssets;

    #[derive(Embed)]
    #[folder = "assets/compiled/"]
    struct StaticAssets;

    pub async fn static_handler(Path(path): Path<String>) -> Response {
        // Try web build assets first, then root assets, then SPA fallback
        if let Some(file) = WebAssets::get(&path) {
            let mime = file.metadata.mimetype();
            return ([(header::CONTENT_TYPE, mime)], file.data).into_response();
        }

        if let Some(file) = StaticAssets::get(&path) {
            let mime = file.metadata.mimetype();
            return ([(header::CONTENT_TYPE, mime)], file.data).into_response();
        }

        // SPA fallback: serve index.html for any unmatched route
        match WebAssets::get("index.html") {
            Some(file) => Html(file.data).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }

    pub async fn index_handler() -> Response {
        match WebAssets::get("index.html") {
            Some(file) => Html(file.data).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }
}

#[cfg(not(debug_assertions))]
pub use prod::{index_handler, static_handler};
