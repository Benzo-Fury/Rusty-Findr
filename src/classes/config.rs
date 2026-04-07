use serde::Deserialize;
use std::path::PathBuf;
use std::{env, fs};
use toml;
use toml::Value;

/// Platform-specific user config directory, equivalent to `dirs::config_dir()`.
fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }
}

const DEFAULT_CONFIG: &str = include_str!("../../assets/compiled/default-config.toml");

#[derive(Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub base_url: String,
    pub trusted_origins: Vec<String>,
}

#[derive(Deserialize)]
pub struct AuthConfig {
    pub secret: String,
}

#[derive(Clone, Deserialize)]
pub struct PathsConfig {
    pub logs: PathBuf,
    pub download: PathBuf,
    pub movies: PathBuf,
    pub series: PathBuf,
}

#[derive(Clone, Deserialize)]
pub struct NamingConfig {
    pub movie_folder: String,
    pub movie_file: String,
    pub series_folder: String,
    pub season_folder: String,
    pub series_file: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StageWeightsConfig {
    pub indexing: f32,
    pub downloading: f32,
    pub sterilizing: f32,
    pub saving: f32,
    pub cleanup: f32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DownloadingConfig {
    pub poll_interval_secs: u64,
    pub min_seeders: i64,
    pub min_seeders_timeout_secs: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JobsConfig {
    pub max_concurrent: usize,
    pub max_retries: u32,
    pub media_extensions: Vec<String>,
    pub stage_weights: StageWeightsConfig,
    pub scoring: ScoringConfig,
    pub downloading: DownloadingConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Clone, Deserialize)]
pub struct QbittorrentConfig {
    pub url: String,
    pub username: String,
    pub password: String,
}

#[derive(Clone, Deserialize)]
pub struct JackettConfig {
    pub url: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScoringConfig {
    pub resolution_weight: f32,
    pub file_size_weight: f32,
    pub seeders_weight: f32,
    pub codec_weight: f32,
    pub release_type_weight: f32,
    pub release_group_weight: f32,
    pub resolutions: Vec<String>,
    pub min_seeders: i32,
    pub ideal_size_gb: f32,
    pub max_4k_size_gb: f32,
    pub bloat_penalty: f32,
    pub blacklisted_release_types: Vec<String>,
    pub reputable_groups: Vec<String>,
}

impl ScoringConfig {
    pub fn blacklisted_release_types(&self) -> Vec<String> {
        self.blacklisted_release_types
            .iter()
            .map(|s| s.to_uppercase())
            .collect()
    }
}

#[derive(Deserialize)]
pub struct ConfigData {
    pub server: ServerConfig,
    pub auth: AuthConfig,
    pub paths: PathsConfig,
    pub naming: NamingConfig,
    pub jobs: JobsConfig,
    pub database: DatabaseConfig,
    pub qbittorrent: QbittorrentConfig,
    pub jackett: JackettConfig,
    pub tmdb: TmdbConfig,
}

#[derive(Clone, Deserialize)]
pub struct TmdbConfig {
    pub api_key: String,
}

pub struct Config {
    pub location: PathBuf,
    pub data: Option<ConfigData>,
}

impl Config {
    pub fn new() -> Config {
        let default_location = config_dir()
            .expect("Could not determine config directory")
            .join("rusty-findr")
            .join("config.toml");

        let location = env::var("CONFIG_LOCATION")
            .map(PathBuf::from)
            .unwrap_or(default_location);

        tracing::debug!("Loading config from {}", location.to_string_lossy());

        let mut instance = Self {
            location,
            data: None,
        };

        instance.load_config();

        return instance;
    }

    fn load_config(&mut self) {
        let path = &self.location;

        if !path.exists() {
            self.write_default();
            eprintln!(
                "Created default config at {}\nUpdate the config with your settings, then re-run.",
                path.display()
            );
            std::process::exit(0);
        }

        let content = fs::read_to_string(path)
            .expect(&format!("Failed to read config from {}", path.display()));

        let default: Value = toml::from_str(DEFAULT_CONFIG)
            .expect("Invalid TOML in default config asset");
        let user: Value = toml::from_str(&content)
            .expect(&format!("Invalid TOML syntax in {}", path.display()));

        let merged = merge_toml(default, user);

        // Write the merged config back so newly added fields persist on disk
        let merged_str = toml::to_string_pretty(&merged)
            .expect("Failed to serialize merged config");
        fs::write(path, &merged_str)
            .expect(&format!("Failed to write merged config to {}", path.display()));

        let data: ConfigData = merged.try_into()
            .expect("Merged config is missing required values");

        self.data = Some(data);
    }

    fn write_default(&self) {
        if let Some(parent) = self.location.parent() {
            fs::create_dir_all(parent).expect(&format!(
                "Failed to create config directory {}",
                parent.display()
            ));
        }

        fs::write(&self.location, DEFAULT_CONFIG).expect(&format!(
            "Failed to write default config to {}",
            self.location.display()
        ));
    }
}

/// Deep-merge two TOML values. `base` provides defaults, `overlay` provides
/// user overrides. User values always win; missing keys are filled from base.
fn merge_toml(base: Value, overlay: Value) -> Value {
    match (base, overlay) {
        (Value::Table(mut base_table), Value::Table(overlay_table)) => {
            for (key, overlay_val) in overlay_table {
                let merged_val = match base_table.remove(&key) {
                    Some(base_val) => merge_toml(base_val, overlay_val),
                    None => overlay_val,
                };
                base_table.insert(key, merged_val);
            }
            Value::Table(base_table)
        }
        // For non-table values, the user's value always wins
        (_, overlay) => overlay,
    }
}
