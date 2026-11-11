//! Concurrent, polite feed fetching with conditional GETs.

use crate::config::{Config, Source};
use crate::model::FeedHealth;
use crate::text::{clean_html, collapse_ws, truncate};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use futures::stream::{self, StreamExt};
use reqwest::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use std::collections::HashMap;
use std::time::Duration;

/// A normalized feed item before clustering.
#[derive(Debug, Clone)]
pub struct RawItem {
    pub source: String,
    pub url: String,
    pub title: String,
    pub snippet: String,
    pub published: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub struct FetchResult {
    pub source: String,
    pub items: Vec<RawItem>,
    pub health: FeedHealth,
}

pub fn client(cfg: &Config) -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(&cfg.site.user_agent)
        .timeout(Duration::from_secs(cfg.site.fetch_timeout_secs))
        .connect_timeout(Duration::from_secs(6))
        .redirect(reqwest::redirect::Policy::limited(5))
        .pool_max_idle_per_host(4)
        .build()?)
}

pub async fn fetch_all(cfg: &Config, prev: &HashMap<String, FeedHealth>) -> Vec<FetchResult> {
    let client = match client(cfg) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("http client: {e}");
            return vec![];
        }
    };
    // Owned jobs keep the futures 'static, which tokio::spawn (serve mode) needs.
    let jobs: Vec<(Source, FeedHealth)> = cfg
        .sources
        .iter()
        .map(|s| (s.clone(), prev.get(&s.id).cloned().unwrap_or_default()))
        .collect();
    stream::iter(jobs)
        .map(move |(src, health)| {
            let client = client.clone();
            async move { fetch_one(client, src, health).await }
        })
        .buffer_unordered(cfg.site.fetch_concurrency.max(1))
        .collect()
        .await
}

async fn fetch_one(client: reqwest::Client, src: Source, mut health: FeedHealth) -> FetchResult {
    let src = &src;
    let started = std::time::Instant::now();
    let outcome = fetch_body(&client, src, &health).await;
    let items = match outcome {
        Ok(Fetched::NotModified) => {
            health.last_ok = Some(Utc::now());
            health.last_error = None;
            health.consecutive_failures = 0;
            vec![]
        }
        Ok(Fetched::Body {
            bytes,
            etag,
            last_modified,
        }) => match parse_feed(&src.id, &bytes) {
            Ok(items) => {
                health.etag = etag;
                health.last_modified = last_modified;
                health.last_ok = Some(Utc::now());
                health.last_error = None;
                health.consecutive_failures = 0;
                health.last_items = items.len();
                items
            }
            Err(e) => fail(&mut health, format!("parse: {e}")),
        },
        Err(e) => fail(&mut health, e.to_string()),
    };
    tracing::debug!(
        "{} → {} items in {:?}",
        src.name,
        items.len(),
        started.elapsed()
    );
    FetchResult {
        source: src.id.clone(),
        items,
        health,
    }
}

fn fail(health: &mut FeedHealth, msg: String) -> Vec<RawItem> {
    health.last_error = Some(truncate(&msg, 200));
    health.consecutive_failures += 1;
    vec![]
}

enum Fetched {
    NotModified,
    Body {
        bytes: Vec<u8>,
        etag: Option<String>,
        last_modified: Option<String>,
    },
}

async fn fetch_body(
    client: &reqwest::Client,
    src: &Source,
    health: &FeedHealth,
) -> Result<Fetched> {
    if let Some(path) = src.url.strip_prefix("file:") {
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| anyhow!("{path}: {e}"))?;
        return Ok(Fetched::Body {
            bytes,
            etag: None,
            last_modified: None,
        });
    }
    let mut req = client.get(&src.url);
    if let Some(e) = &health.etag {
        req = req.header(IF_NONE_MATCH, e);
    }
    if let Some(lm) = &health.last_modified {
        req = req.header(IF_MODIFIED_SINCE, lm);
    }
    let resp = req.send().await?;
    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(Fetched::NotModified);
    }
    if !resp.status().is_success() {
        return Err(anyhow!("HTTP {}", resp.status()));
    }
    let h = resp.headers();
    let etag = h.get(ETAG).and_then(|v| v.to_str().ok()).map(String::from);
    let last_modified = h
        .get(LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let bytes = resp.bytes().await?.to_vec();
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(anyhow!("feed larger than 8 MB"));
    }
    Ok(Fetched::Body {
        bytes,
        etag,
        last_modified,
    })
}

pub fn parse_feed(source_id: &str, bytes: &[u8]) -> Result<Vec<RawItem>> {
    let feed = feed_rs::parser::parse(bytes)?;
    let mut out = Vec::with_capacity(feed.entries.len());
    for e in feed.entries {
        let title = e.title.map(|t| clean_html(&t.content)).unwrap_or_default();
        if title.len() < 8 {
            continue;
        }
        let link = e
            .links
            .iter()
            .find(|l| l.rel.as_deref().map(|r| r == "alternate").unwrap_or(true))
            .or_else(|| e.links.first())
            .map(|l| l.href.clone())
            .or_else(|| {
                // Some RSS feeds only put the URL in <guid>.
                (e.id.starts_with("http://") || e.id.starts_with("https://")).then(|| e.id.clone())
            });
        let Some(link) = link else { continue };
        let Some(url) = canonical_url(&link) else {
            continue;
        };
        let summary = e
            .summary
            .map(|s| s.content)
            .or_else(|| e.content.and_then(|c| c.body))
            .map(|s| clean_html(&s))
            .unwrap_or_default();
        let snippet = tidy_snippet(&title, &summary);
        let published = e.published.or(e.updated);
        out.push(RawItem {
            source: source_id.to_string(),
            url,
            title: collapse_ws(&title),
            snippet,
            published,
        });
    }
    Ok(out)
}

/// Remove boilerplate and anything that just repeats the headline.
fn tidy_snippet(title: &str, summary: &str) -> String {
    let mut s = summary.trim().to_string();
    for junk in [
        "Continue reading...",
        "Continue reading…",
        "Read more",
        "[…]",
        "[...]",
        "The post ",
    ] {
        if let Some(pos) = s.find(junk) {
            if pos > 40 {
                s.truncate(pos);
            }
        }
    }
    let s = s.trim();
    if s.is_empty() || s.eq_ignore_ascii_case(title) || title.starts_with(s) {
        return String::new();
    }
    // Hacker News style "Article URL: … Comments URL: …" carries no prose.
    if s.starts_with("Article URL:") || s.starts_with("Comments") {
        return String::new();
    }
    truncate(s, 300)
}

/// Strip tracking parameters and fragments so the same story URL dedupes.
pub fn canonical_url(raw: &str) -> Option<String> {
    let mut u = url::Url::parse(raw.trim()).ok()?;
    if u.scheme() != "http" && u.scheme() != "https" {
        return None;
    }
    u.set_fragment(None);
    let kept: Vec<(String, String)> = u
        .query_pairs()
        .filter(|(k, _)| {
            let k = k.to_ascii_lowercase();
            !(k.starts_with("utm_")
                || matches!(
                    k.as_str(),
                    "cmp"
                        | "ref"
                        | "rss"
                        | "ns_mchannel"
                        | "ns_source"
                        | "ns_campaign"
                        | "ocid"
                        | "at_medium"
                        | "at_campaign"
                        | "fbclid"
                        | "gclid"
                        | "src"
                        | "taid"
                        | "mbid"
                        | "rssfeed"
                ))
        })
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    if kept.is_empty() {
        u.set_query(None);
    } else {
        u.query_pairs_mut().clear().extend_pairs(kept);
    }
    Some(u.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RSS: &str = r#"<?xml version="1.0"?>
<rss version="2.0"><channel><title>Example</title>
<item><title>Storm makes landfall on the coast</title><link>https://example.com/a?utm_source=rss&amp;id=4#top</link>
<description>&lt;p&gt;The storm brought &lt;b&gt;heavy&lt;/b&gt; rain.&lt;/p&gt;</description>
<pubDate>Tue, 10 Nov 2026 10:00:00 GMT</pubDate></item>
<item><title>Short</title><link>https://example.com/b</link></item>
</channel></rss>"#;

    const ATOM: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom"><title>Ex</title><id>x</id><updated>2026-11-10T10:00:00Z</updated>
<entry><title>Chipmaker unveils faster processor</title><id>tag:x,1</id>
<link rel="alternate" href="https://example.org/chip"/><updated>2026-11-10T09:00:00Z</updated>
<summary>Chipmaker unveils faster processor</summary></entry></feed>"#;

    #[test]
    fn parses_rss() {
        let items = parse_feed("s1", RSS.as_bytes()).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].url, "https://example.com/a?id=4");
        assert_eq!(items[0].snippet, "The storm brought heavy rain.");
        assert!(items[0].published.is_some());
    }

    #[test]
    fn parses_atom_and_drops_repeated_summary() {
        let items = parse_feed("s2", ATOM.as_bytes()).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].url, "https://example.org/chip");
        assert_eq!(items[0].snippet, "");
    }

    #[test]
    fn canonicalizes() {
        assert_eq!(
            canonical_url("https://x.com/p?utm_medium=a&b=2#c").unwrap(),
            "https://x.com/p?b=2"
        );
        assert!(canonical_url("javascript:alert(1)").is_none());
    }
}
