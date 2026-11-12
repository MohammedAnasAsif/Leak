use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// The sections shown in every client. Order here is the order in the nav.
pub const CATEGORIES: &[(&str, &str)] = &[
    ("world", "World"),
    ("politics", "Politics"),
    ("finance", "Finance"),
    ("tech", "Tech"),
    ("science", "Science"),
    ("health", "Health"),
    ("sports", "Sports"),
    ("entertainment", "Entertainment"),
    ("climate", "Climate"),
];

pub fn is_category(slug: &str) -> bool {
    CATEGORIES.iter().any(|(s, _)| *s == slug)
}

/// One story, presented by the clients as a blog-style post.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    /// Stable id derived from the canonical URL.
    pub id: String,
    pub title: String,
    /// Plain-text summary supplied by the publisher's feed (trimmed).
    pub summary: String,
    /// Link to the full story on the publisher's site.
    pub url: String,
    pub image: Option<String>,
    pub source: String,
    pub source_domain: String,
    pub category: String,
    pub author: Option<String>,
    /// When the publisher says it was published.
    pub published: DateTime<Utc>,
    /// When Leak first saw it.
    pub detected: DateTime<Utc>,
    /// Estimated read time of the full story, in minutes.
    pub read_minutes: u32,
    /// How the story arrived: "push" (WebSub), "poll" (RSS/Atom) or "gdelt".
    pub via: String,
    /// AI-written "savage" version of the headline (original stays in `title`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub savage_title: Option<String>,
}

impl Article {
    /// Seconds between publication and detection (clamped at 0).
    pub fn detection_lag_secs(&self) -> i64 {
        (self.detected - self.published).num_seconds().max(0)
    }
}

/// Health of one configured source, exposed at /api/sources.
#[derive(Debug, Clone, Serialize, Default)]
pub struct SourceStatus {
    pub id: String,
    pub name: String,
    pub url: String,
    pub category: String,
    pub mode: String,
    pub last_ok: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub last_checked: Option<DateTime<Utc>>,
    pub items_seen: u64,
    pub new_items: u64,
}

/// What live clients receive over Server-Sent Events.
#[derive(Debug, Clone)]
pub enum LiveEvent {
    New(Arc<Article>),
    Update(Arc<Article>),
}
