use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Section {
    pub id: String,
    pub name: String,
    pub color: String,
    #[serde(default)]
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Distribution {
    #[serde(default = "d_min_sources")]
    pub min_sources: usize,
    #[serde(default = "d_max_posts")]
    pub max_posts_per_run: usize,
    #[serde(default = "d_max_age")]
    pub max_story_age_minutes: i64,
}
fn d_min_sources() -> usize {
    3
}
fn d_max_posts() -> usize {
    3
}
fn d_max_age() -> i64 {
    180
}
impl Default for Distribution {
    fn default() -> Self {
        Self {
            min_sources: 3,
            max_posts_per_run: 3,
            max_story_age_minutes: 180,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SiteConfig {
    pub name: String,
    pub tagline: String,
    pub description: String,
    pub base_url: String,
    #[serde(default)]
    pub repo_url: String,
    #[serde(default = "d_lang")]
    pub language: String,
    #[serde(default = "d_retention")]
    pub retention_hours: i64,
    #[serde(default = "d_item_age")]
    pub max_item_age_hours: i64,
    #[serde(default = "d_cluster_window")]
    pub cluster_window_hours: i64,
    #[serde(default = "d_conc")]
    pub fetch_concurrency: usize,
    #[serde(default = "d_timeout")]
    pub fetch_timeout_secs: u64,
    pub user_agent: String,
    #[serde(default)]
    pub distribution: Distribution,
    pub sections: Vec<Section>,
}
fn d_lang() -> String {
    "en".into()
}
fn d_retention() -> i64 {
    168
}
fn d_item_age() -> i64 {
    36
}
fn d_cluster_window() -> i64 {
    36
}
fn d_conc() -> usize {
    24
}
fn d_timeout() -> u64 {
    12
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Source {
    pub name: String,
    pub url: String,
    pub section: String,
    pub country: String,
    pub region: String,
    #[serde(default = "d_weight")]
    pub weight: f64,
    /// Stable id derived from the URL (filled in after loading).
    #[serde(default)]
    pub id: String,
}
fn d_weight() -> f64 {
    1.0
}

#[derive(Debug, Deserialize)]
struct SourcesFile {
    source: Vec<Source>,
}

pub const REGIONS: &[&str] = &[
    "Africa",
    "Americas",
    "Asia",
    "Europe",
    "Middle East",
    "Oceania",
    "Global",
];

#[derive(Debug, Clone)]
pub struct Config {
    pub site: SiteConfig,
    pub sources: Vec<Source>,
}

impl Config {
    pub fn load(dir: &Path) -> Result<Self> {
        let site_raw =
            std::fs::read_to_string(dir.join("site.toml")).context("reading site.toml")?;
        let site: SiteConfig = toml::from_str(&site_raw).context("parsing site.toml")?;
        let src_raw =
            std::fs::read_to_string(dir.join("sources.toml")).context("reading sources.toml")?;
        let file: SourcesFile = toml::from_str(&src_raw).context("parsing sources.toml")?;
        let mut cfg = Config {
            site,
            sources: file.source,
        };
        cfg.finish()?;
        Ok(cfg)
    }

    /// Validate and fill derived fields. Public so tests and the demo can build configs in code.
    pub fn finish(&mut self) -> Result<()> {
        self.site.base_url = self.site.base_url.trim_end_matches('/').to_string();
        let section_ids: Vec<&str> = self.site.sections.iter().map(|s| s.id.as_str()).collect();
        if section_ids.is_empty() {
            bail!("site.toml needs at least one [[sections]] entry");
        }
        let mut seen = std::collections::HashSet::new();
        for s in &mut self.sources {
            if !section_ids.contains(&s.section.as_str()) {
                bail!("source '{}' uses unknown section '{}'", s.name, s.section);
            }
            if !REGIONS.contains(&s.region.as_str()) {
                bail!(
                    "source '{}' uses unknown region '{}' (use one of {:?})",
                    s.name,
                    s.region,
                    REGIONS
                );
            }
            s.weight = s.weight.clamp(0.3, 2.0);
            s.id = short_hash(&s.url, 8);
            if !seen.insert(s.id.clone()) {
                bail!("duplicate source url: {}", s.url);
            }
        }
        Ok(())
    }

    pub fn source(&self, id: &str) -> Option<&Source> {
        self.sources.iter().find(|s| s.id == id)
    }

    pub fn section(&self, id: &str) -> Option<&Section> {
        self.site.sections.iter().find(|s| s.id == id)
    }
}

/// Short, stable, URL-safe hex hash.
pub fn short_hash(input: &str, len: usize) -> String {
    let h = blake3::hash(input.as_bytes());
    h.to_hex()[..len].to_string()
}
