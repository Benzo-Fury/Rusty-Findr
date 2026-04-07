use std::future::Future;
use std::pin::Pin;

use crate::functions::find_binary::find_binary;

/// A dependency that is validated at startup before the application begins processing.
///
/// Prerequisites represent external services, binaries, or conditions that the app
/// depends on. Each prerequisite carries its own async check callback, so the runner
/// does not need to know how to validate different kinds of dependencies.
///
/// Prerequisites can be marked as required or optional. Required prerequisites will
/// cause the app to exit on failure, while optional ones only produce a warning.
pub struct Prerequisite {
    /// Human-readable name for logging (e.g. "PostgreSQL", "VPN").
    pub name: &'static str,
    /// When `true`, the app exits if this check fails.
    /// When `false`, a warning is logged but startup continues.
    pub required: bool,
    /// Async callback that determines availability. Returns `Ok(())` when the
    /// prerequisite is satisfied, or `Err(reason)` describing why it is not.
    pub check: Box<dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), String>>>>>,
    /// Setup instructions displayed to the user when the check fails.
    pub help: &'static str,
}

pub async fn check_service(url: impl AsRef<str>) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    client
        .get(url.as_ref())
        .send()
        .await
        .map(|_| ())
        .map_err(|_| format!("Could not reach {}", url.as_ref()))
}

pub async fn check_tcp(host: &str, port: u16) -> Result<(), String> {
    let addr = format!("{host}:{port}");
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::net::TcpStream::connect(&addr),
    )
    .await
    .map_err(|_| format!("Connection to {addr} timed out"))?
    .map(|_| ())
    .map_err(|_| format!("Could not connect to {addr}"))
}

pub async fn check_binary(name: &str) -> Result<(), String> {
    match find_binary(name) {
        Some(path) => {
            tracing::debug!("Found {name} at {}", path.display());
            Ok(())
        }
        None => Err(format!("'{name}' not found in PATH")),
    }
}

/// Checks if the server's public IP looks like a VPN/proxy by querying ip-api.com.
pub async fn check_vpn() -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let resp = client
        .get("http://ip-api.com/json/?fields=proxy,hosting")
        .send()
        .await
        .map_err(|e| format!("Could not reach ip-api.com: {e}"))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("Invalid response from ip-api.com: {e}"))?;

    let is_proxy = resp.get("proxy").and_then(|v| v.as_bool()).unwrap_or(false);
    let is_hosting = resp.get("hosting").and_then(|v| v.as_bool()).unwrap_or(false);

    if is_proxy || is_hosting {
        Ok(())
    } else {
        Err("No VPN detected. We recommend using a VPN for privacy.".to_string())
    }
}
