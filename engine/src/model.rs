use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// One item from one outlet's feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub id: String,
    pub source: String,
    pub url: String,
    pub title: String,
    #[serde(default)]
    pub snippet: String,
    /// When the outlet says it published (clamped to never be in the future).
    pub published: DateTime<Utc>,
    /// When Leak first saw it.
    pub seen: DateTime<Utc>,
    pub story: String,
}

/// A cluster of articles about the same event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Story {
    pub id: String,
    /// Frozen at creation so links never change.
    pub slug: String,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub articles: Vec<String>,
    /// Set once the distribution bots have posted it.
    #[serde(default)]
    pub posted: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeedHealth {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub last_ok: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub last_items: usize,
    pub consecutive_failures: u32,
    /// Median minutes between an outlet publishing and Leak seeing it.
    #[serde(default)]
    pub lag_minutes: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub version: u32,
    pub updated: DateTime<Utc>,
    pub articles: HashMap<String, Article>,
    pub stories: HashMap<String, Story>,
    pub feeds: HashMap<String, FeedHealth>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            updated: Utc::now(),
            articles: HashMap::new(),
            stories: HashMap::new(),
            feeds: HashMap::new(),
        }
    }
}

impl State {
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        match serde_json::from_slice::<State>(&raw) {
            Ok(s) => Ok(s),
            Err(e) => {
                tracing::warn!("state file unreadable ({e}); starting fresh");
                Ok(Self::default())
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec(self)?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Drop stories (and their articles) older than the retention window.
    pub fn prune(&mut self, now: DateTime<Utc>, retention_hours: i64) {
        let cutoff = now - chrono::Duration::hours(retention_hours);
        let dead: Vec<String> = self
            .stories
            .values()
            .filter(|s| s.updated < cutoff)
            .map(|s| s.id.clone())
            .collect();
        for id in dead {
            if let Some(story) = self.stories.remove(&id) {
                for a in story.articles {
                    self.articles.remove(&a);
                }
            }
        }
        // Orphans (should not happen, but keep state tidy).
        let stories = &self.stories;
        self.articles.retain(|_, a| stories.contains_key(&a.story));
    }
}
