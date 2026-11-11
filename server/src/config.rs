use serde::Deserialize;
use std::{path::Path, time::Duration};

/// `sources.toml`: every feed and GDELT query Leak watches.
#[derive(Debug, Clone, Deserialize)]
pub struct SourcesFile {
    #[serde(default)]
    pub settings: Settings,
    #[serde(default, rename = "feed")]
    pub feeds: Vec<FeedSource>,
    #[serde(default, rename = "gdelt")]
    pub gdelt: Vec<GdeltQuery>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Normal polling interval for a feed, seconds.
    pub poll_secs: u64,
    /// Interval for feeds marked `fast = true` (wires, breaking desks).
    pub fast_poll_secs: u64,
    /// Interval between GDELT sweeps (GDELT asks for ≤1 request / 5s).
    pub gdelt_secs: u64,
    /// Max stories kept in memory.
    pub max_articles: usize,
    /// Max concurrent outbound HTTP requests.
    pub concurrency: usize,
    /// Ignore stories older than this when first seen, hours.
    pub max_age_hours: i64,
    /// Look up og:image for stories that arrive without a picture.
    pub enrich_images: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            poll_secs: 60,
            fast_poll_secs: 20,
            gdelt_secs: 120,
            max_articles: 6000,
            concurrency: 48,
            max_age_hours: 48,
            enrich_images: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeedSource {
    pub name: String,
    pub url: String,
    pub category: String,
    #[serde(default)]
    pub fast: bool,
}

impl FeedSource {
    pub fn id(&self) -> String {
        crate::util::short_hash(&self.url)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GdeltQuery {
    pub category: String,
    pub query: String,
}

impl SourcesFile {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
        let mut parsed: SourcesFile = toml::from_str(&text)?;
        parsed.feeds.retain(|f| {
            let ok = crate::model::is_category(&f.category);
            if !ok {
                tracing::warn!("skipping feed {} with unknown category {}", f.name, f.category);
            }
            ok
        });
        Ok(parsed)
    }
}

/// Process-level settings from environment variables.
#[derive(Debug, Clone)]
pub struct Env {
    pub bind: String,
    pub sources_path: String,
    pub web_dir: String,
    pub data_dir: String,
    /// Public https base URL of this server. Enables WebSub push subscriptions.
    pub public_url: Option<String>,
    pub disable_gdelt: bool,
}

impl Env {
    pub fn from_env() -> Self {
        let port = std::env::var("PORT").unwrap_or_else(|_| "8080".into());
        Self {
            bind: std::env::var("LEAK_BIND").unwrap_or_else(|_| format!("0.0.0.0:{port}")),
            sources_path: std::env::var("LEAK_SOURCES").unwrap_or_else(|_| "sources.toml".into()),
            web_dir: std::env::var("LEAK_WEB_DIR").unwrap_or_else(|_| "../web".into()),
            data_dir: std::env::var("LEAK_DATA_DIR").unwrap_or_else(|_| "data".into()),
            public_url: std::env::var("LEAK_PUBLIC_URL")
                .ok()
                .map(|s| s.trim_end_matches('/').to_string())
                .filter(|s| s.starts_with("https://")),
            disable_gdelt: std::env::var("LEAK_DISABLE_GDELT").is_ok(),
        }
    }
}

pub fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}
