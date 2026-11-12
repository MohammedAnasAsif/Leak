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
    /// Savage headlines; None when no LLM key is configured.
    pub llm: Option<LlmConfig>,
}

/// LLM used to write savage headlines (see spice.rs).
#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// "anthropic" or "openai" (any OpenAI-compatible chat API).
    pub provider: String,
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub daily_limit: u32,
    pub max_age_hours: i64,
}

impl LlmConfig {
    fn from_env() -> Option<Self> {
        let var = |k: &str| std::env::var(k).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let api_key = var("LEAK_LLM_API_KEY").or_else(|| var("ANTHROPIC_API_KEY"))?;
        if matches!(var("LEAK_SAVAGE").as_deref(), Some("0" | "off" | "false")) {
            return None;
        }
        let provider = var("LEAK_LLM_PROVIDER").unwrap_or_else(|| "anthropic".into()).to_lowercase();
        let provider = if provider == "anthropic" { provider } else { "openai".to_string() };
        let (default_model, default_base) = if provider == "anthropic" {
            ("claude-haiku-5-5", "https://api.anthropic.com")
        } else {
            ("gpt-4o-mini", "https://api.openai.com/v1")
        };
        Some(Self {
            model: var("LEAK_LLM_MODEL").unwrap_or_else(|| default_model.into()),
            base_url: var("LEAK_LLM_BASE_URL").unwrap_or_else(|| default_base.into()).trim_end_matches('/').to_string(),
            daily_limit: var("LEAK_SAVAGE_DAILY_LIMIT").and_then(|v| v.parse().ok()).unwrap_or(3000),
            max_age_hours: var("LEAK_SAVAGE_MAX_AGE_HOURS").and_then(|v| v.parse().ok()).unwrap_or(12),
            provider,
            api_key,
        })
    }
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
            llm: LlmConfig::from_env(),
        }
    }
}

pub fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}
