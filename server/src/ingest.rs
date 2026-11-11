//! Getting stories in as fast as the sources allow:
//!  * RSS/Atom polling with conditional GETs (ETag / Last-Modified), so a
//!    poll that finds nothing costs a few hundred bytes and we can poll often.
//!  * WebSub (PubSubHubbub) push: when a feed advertises a hub and the server
//!    has a public https URL, the hub POSTs new items to us the moment they are
//!    published. Polling then drops to a slow safety net.
//!  * GDELT DOC API sweeps, which catch stories from tens of thousands of
//!    outlets worldwide that we don't have feeds for.
//!  * og:image enrichment for stories that arrive without a picture.

use crate::config::{secs, Env, FeedSource, GdeltQuery, Settings};
use crate::model::{Article, SourceStatus};
use crate::store::Store;
use crate::util;
use chrono::{DateTime, TimeZone, Utc};
use hmac::{Hmac, Mac};
use parking_lot::Mutex;
use rand::Rng;
use reqwest::header::{HeaderMap, HeaderValue, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

pub const USER_AGENT: &str =
    "Mozilla/5.0 (compatible; LeakNews/1.0; +https://github.com/MohammedAnasAsif/Leak)";

pub struct Ingest {
    pub store: Arc<Store>,
    pub http: reqwest::Client,
    pub settings: Settings,
    pub env: Env,
    pub permits: Arc<Semaphore>,
    pub websub: Mutex<HashMap<String, WebSubState>>,
    pub feeds: HashMap<String, FeedSource>,
    enrich_tried: Mutex<HashSet<String>>,
}

#[derive(Debug, Clone)]
pub struct WebSubState {
    #[allow(dead_code)]
    pub hub: String,
    pub topic: String,
    pub secret: String,
    pub verified_until: Option<DateTime<Utc>>,
}

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(6))
        .pool_idle_timeout(Duration::from_secs(90))
        .gzip(true)
        .brotli(true)
        .deflate(true)
        .build()
        .expect("http client")
}

impl Ingest {
    pub fn new(store: Arc<Store>, settings: Settings, env: Env, feeds: &[FeedSource]) -> Arc<Self> {
        {
            let mut s = store.sources.write();
            for f in feeds {
                s.insert(
                    f.id(),
                    SourceStatus {
                        id: f.id(),
                        name: f.name.clone(),
                        url: f.url.clone(),
                        category: f.category.clone(),
                        mode: "poll".into(),
                        ..Default::default()
                    },
                );
            }
        }
        Arc::new(Self {
            permits: Arc::new(Semaphore::new(settings.concurrency.max(1))),
            store,
            http: http_client(),
            settings,
            env,
            websub: Mutex::new(HashMap::new()),
            feeds: feeds.iter().map(|f| (f.id(), f.clone())).collect(),
            enrich_tried: Mutex::new(HashSet::new()),
        })
    }

    /// Start one polling task per feed, staggered so we don't burst at boot.
    pub fn spawn_pollers(self: &Arc<Self>) {
        let n = self.feeds.len().max(1) as u64;
        for (i, feed) in self.feeds.values().cloned().enumerate() {
            let me = self.clone();
            let stagger = Duration::from_millis((i as u64 * 4000) / n);
            tokio::spawn(async move {
                tokio::time::sleep(stagger).await;
                me.poll_loop(feed).await;
            });
        }
    }

    async fn poll_loop(self: Arc<Self>, feed: FeedSource) {
        let id = feed.id();
        let mut etag: Option<String> = None;
        let mut last_modified: Option<String> = None;
        let mut first = true;
        let mut failures: u32 = 0;
        loop {
            let pushed = self
                .websub
                .lock()
                .get(&id)
                .and_then(|w| w.verified_until)
                .map_or(false, |t| t > Utc::now());
            let base = if pushed {
                300 // push is live; polling is only a safety net
            } else if feed.fast {
                self.settings.fast_poll_secs
            } else {
                self.settings.poll_secs
            };
            match self.poll_once(&feed, &mut etag, &mut last_modified, first).await {
                Ok(()) => {
                    first = false;
                    failures = 0;
                }
                Err(e) => {
                    failures += 1;
                    let msg = e.to_string();
                    tracing::debug!(feed = %feed.name, "poll failed: {msg}");
                    self.store.update_source(&id, |s| {
                        s.last_error = Some(msg);
                        s.last_checked = Some(Utc::now());
                    });
                }
            }
            // Back off on repeated failure (max ~15 min), add jitter so feeds spread out.
            let backoff = base.saturating_mul(1u64 << failures.min(4)).min(900);
            let jitter = rand::thread_rng().gen_range(0..=(backoff / 5).max(1));
            tokio::time::sleep(secs(backoff + jitter)).await;
        }
    }

    async fn poll_once(
        &self,
        feed: &FeedSource,
        etag: &mut Option<String>,
        last_modified: &mut Option<String>,
        first: bool,
    ) -> anyhow::Result<()> {
        let _permit = self.permits.acquire().await?;
        let mut headers = HeaderMap::new();
        if let Some(e) = etag.as_deref().and_then(|v| HeaderValue::from_str(v).ok()) {
            headers.insert(IF_NONE_MATCH, e);
        }
        if let Some(l) = last_modified.as_deref().and_then(|v| HeaderValue::from_str(v).ok()) {
            headers.insert(IF_MODIFIED_SINCE, l);
        }
        let resp = self.http.get(&feed.url).headers(headers).send().await?;
        let id = feed.id();
        if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
            self.store.update_source(&id, |s| {
                s.last_ok = Some(Utc::now());
                s.last_checked = s.last_ok;
                s.last_error = None;
            });
            return Ok(());
        }
        if !resp.status().is_success() {
            anyhow::bail!("HTTP {}", resp.status());
        }
        *etag = resp.headers().get(ETAG).and_then(|v| v.to_str().ok()).map(str::to_string);
        *last_modified =
            resp.headers().get(LAST_MODIFIED).and_then(|v| v.to_str().ok()).map(str::to_string);
        let body = resp.bytes().await?;
        drop(_permit);
        let (seen, added, hub, topic) = self.ingest_feed_bytes(feed, &body, "poll", !first)?;
        self.store.update_source(&id, |s| {
            s.last_ok = Some(Utc::now());
            s.last_checked = s.last_ok;
            s.last_error = None;
            s.items_seen += seen as u64;
            s.new_items += added as u64;
        });
        if let (Some(hub), Some(public)) = (hub, self.env.public_url.clone()) {
            self.maybe_subscribe(feed, hub, topic.unwrap_or_else(|| feed.url.clone()), public).await;
        }
        Ok(())
    }

    /// Parse a feed document and store its new items.
    /// Returns (items_seen, items_added, websub_hub, websub_self_topic).
    pub fn ingest_feed_bytes(
        &self,
        feed: &FeedSource,
        body: &[u8],
        via: &str,
        announce: bool,
    ) -> anyhow::Result<(usize, usize, Option<String>, Option<String>)> {
        let parsed = feed_rs::parser::parse(body)?;
        let hub = parsed
            .links
            .iter()
            .find(|l| l.rel.as_deref() == Some("hub"))
            .map(|l| l.href.clone());
        let topic = parsed
            .links
            .iter()
            .find(|l| l.rel.as_deref() == Some("self"))
            .map(|l| l.href.clone());
        let now = Utc::now();
        let oldest = now - chrono::Duration::hours(self.settings.max_age_hours);
        let mut added = 0;
        let seen = parsed.entries.len();
        for entry in parsed.entries {
            let Some(link) = entry
                .links
                .iter()
                .find(|l| l.rel.as_deref().map_or(true, |r| r == "alternate"))
                .or_else(|| entry.links.first())
                .map(|l| l.href.clone())
                .or_else(|| entry.id.starts_with("http").then(|| entry.id.clone()))
            else {
                continue;
            };
            let title = entry.title.as_ref().map(|t| util::strip_html(&t.content)).unwrap_or_default();
            if title.len() < 8 {
                continue;
            }
            let published = entry.published.or(entry.updated).unwrap_or(now).min(now);
            if published < oldest {
                continue;
            }
            let url_key = util::canonical_url(&link);
            if self.store.has_url(&url_key) {
                continue;
            }
            let raw_summary = entry
                .summary
                .as_ref()
                .map(|s| s.content.clone())
                .or_else(|| entry.content.as_ref().and_then(|c| c.body.clone()))
                .unwrap_or_default();
            let content_html = entry.content.as_ref().and_then(|c| c.body.clone()).unwrap_or_default();
            let summary_text = util::strip_html(&raw_summary);
            let words = util::strip_html(&content_html).split_whitespace().count().max(
                summary_text.split_whitespace().count(),
            );
            let mut summary = util::truncate_words(&summary_text, 420);
            if summary.eq_ignore_ascii_case(&title) {
                summary.clear();
            }
            let image = pick_image(&entry).or_else(|| {
                util::first_img(&raw_summary).or_else(|| util::first_img(&content_html))
            });
            let author = entry
                .authors
                .first()
                .map(|p| util::strip_html(&p.name))
                .filter(|n| !n.is_empty() && !n.contains('@') && n.len() < 80);
            let article = Article {
                id: util::short_hash(&url_key),
                title,
                summary,
                url: link.clone(),
                image,
                source: feed.name.clone(),
                source_domain: util::domain_of(&link),
                category: feed.category.clone(),
                author,
                published,
                detected: now,
                read_minutes: util::read_minutes(words),
                via: via.to_string(),
            };
            let tkey = util::title_key(&article.title);
            if self.store.insert(article, url_key, tkey, announce) {
                added += 1;
            }
        }
        Ok((seen, added, hub, topic))
    }

    // ---------------------------------------------------------------- WebSub

    async fn maybe_subscribe(&self, feed: &FeedSource, hub: String, topic: String, public: String) {
        let id = feed.id();
        {
            let mut w = self.websub.lock();
            if let Some(state) = w.get(&id) {
                let fresh = state.verified_until.map_or(false, |t| t - Utc::now() > chrono::Duration::hours(12));
                if fresh || state.verified_until.is_none() {
                    return; // already subscribed, or a request is pending verification
                }
            }
            let secret: String = {
                let mut r = rand::thread_rng();
                (0..32).map(|_| format!("{:x}", r.gen_range(0..16u8))).collect()
            };
            w.insert(id.clone(), WebSubState { hub: hub.clone(), topic: topic.clone(), secret, verified_until: None });
        }
        let secret = self.websub.lock().get(&id).map(|s| s.secret.clone()).unwrap_or_default();
        let callback = format!("{public}/websub/{id}");
        let form = [
            ("hub.mode", "subscribe"),
            ("hub.topic", topic.as_str()),
            ("hub.callback", callback.as_str()),
            ("hub.lease_seconds", "864000"),
            ("hub.secret", secret.as_str()),
        ];
        match self.http.post(&hub).form(&form).send().await {
            Ok(r) if r.status().is_success() => {
                tracing::info!(feed = %feed.name, %hub, "WebSub subscription requested");
            }
            Ok(r) => {
                tracing::debug!(feed = %feed.name, "WebSub hub refused: {}", r.status());
                self.websub.lock().remove(&id);
            }
            Err(e) => {
                tracing::debug!(feed = %feed.name, "WebSub hub error: {e}");
                self.websub.lock().remove(&id);
            }
        }
    }

    /// Hub verification (GET). Returns the challenge to echo, if valid.
    pub fn websub_verify(&self, id: &str, mode: &str, topic: &str, challenge: &str, lease: Option<i64>) -> Option<String> {
        let mut w = self.websub.lock();
        let state = w.get_mut(id)?;
        if state.topic != topic {
            return None;
        }
        match mode {
            "subscribe" => {
                state.verified_until = Some(Utc::now() + chrono::Duration::seconds(lease.unwrap_or(86_400)));
                self.store.update_source(id, |s| s.mode = "push".into());
                tracing::info!(feed = id, "WebSub push active");
                Some(challenge.to_string())
            }
            "unsubscribe" => Some(challenge.to_string()),
            _ => None,
        }
    }

    /// Content delivery (POST) from a hub. Signature must match our secret.
    pub fn websub_deliver(&self, id: &str, signature: Option<&str>, body: &[u8]) -> bool {
        let Some(feed) = self.feeds.get(id).cloned() else { return false };
        let Some(secret) = self.websub.lock().get(id).map(|s| s.secret.clone()) else { return false };
        let Some(sig) = signature else { return false };
        let Some((algo, hexsig)) = sig.split_once('=') else { return false };
        let Ok(expected) = hex::decode(hexsig) else { return false };
        let ok = match algo {
            "sha1" => {
                let mut m = Hmac::<sha1::Sha1>::new_from_slice(secret.as_bytes()).unwrap();
                m.update(body);
                m.verify_slice(&expected).is_ok()
            }
            "sha256" => {
                let mut m = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).unwrap();
                m.update(body);
                m.verify_slice(&expected).is_ok()
            }
            _ => false,
        };
        if !ok {
            tracing::warn!(feed = id, "WebSub delivery with bad signature dropped");
            return true; // per spec, acknowledge but ignore
        }
        if let Ok((_, added, _, _)) = self.ingest_feed_bytes(&feed, body, "push", true) {
            self.store.update_source(id, |s| {
                s.new_items += added as u64;
                s.last_ok = Some(Utc::now());
            });
        }
        true
    }

    // ---------------------------------------------------------------- GDELT

    pub fn spawn_gdelt(self: &Arc<Self>, queries: Vec<GdeltQuery>) {
        if queries.is_empty() || self.env.disable_gdelt {
            return;
        }
        let me = self.clone();
        tokio::spawn(async move {
            let mut first = true;
            loop {
                for q in &queries {
                    if let Err(e) = me.gdelt_once(q, !first).await {
                        tracing::debug!(category = %q.category, "GDELT: {e}");
                    }
                    // GDELT's published limit is one request every 5 seconds.
                    tokio::time::sleep(Duration::from_secs(6)).await;
                }
                first = false;
                tokio::time::sleep(secs(me.settings.gdelt_secs)).await;
            }
        });
    }

    async fn gdelt_once(&self, q: &GdeltQuery, announce: bool) -> anyhow::Result<()> {
        #[derive(serde::Deserialize)]
        struct Resp {
            #[serde(default)]
            articles: Vec<GArticle>,
        }
        #[derive(serde::Deserialize)]
        struct GArticle {
            url: String,
            title: String,
            seendate: String,
            #[serde(default)]
            socialimage: String,
            #[serde(default)]
            domain: String,
        }
        let mut url = url::Url::parse("https://api.gdeltproject.org/api/v2/doc/doc")?;
        url.query_pairs_mut()
            .append_pair("query", &q.query)
            .append_pair("mode", "artlist")
            .append_pair("maxrecords", "60")
            .append_pair("format", "json")
            .append_pair("timespan", "30min")
            .append_pair("sort", "datedesc");
        let text = {
            let _p = self.permits.acquire().await?;
            self.http.get(url).send().await?.text().await?
        };
        let resp: Resp = serde_json::from_str(&text)
            .map_err(|_| anyhow::anyhow!("non-JSON reply: {}", text.chars().take(80).collect::<String>()))?;
        let now = Utc::now();
        for g in resp.articles {
            let url_key = util::canonical_url(&g.url);
            if self.store.has_url(&url_key) {
                continue;
            }
            let published = parse_gdelt_date(&g.seendate).unwrap_or(now).min(now);
            let title = util::strip_html(&g.title);
            if title.len() < 12 {
                continue;
            }
            let source = pretty_source(&g.domain);
            let article = Article {
                id: util::short_hash(&url_key),
                summary: String::new(),
                url: g.url.clone(),
                image: Some(g.socialimage).filter(|s| s.starts_with("http")),
                source,
                source_domain: util::domain_of(&g.url),
                category: q.category.clone(),
                author: None,
                published,
                detected: now,
                read_minutes: util::read_minutes(0),
                via: "gdelt".into(),
                title,
            };
            let tkey = util::title_key(&article.title);
            self.store.insert(article, url_key, tkey, announce);
        }
        Ok(())
    }

    // ---------------------------------------------------------- image enrich

    pub fn spawn_enricher(self: &Arc<Self>) {
        if !self.settings.enrich_images {
            return;
        }
        let me = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(8)).await;
                let candidates: Vec<_> = {
                    let tried = me.enrich_tried.lock();
                    me.store
                        .articles_without_image(200)
                        .into_iter()
                        .filter(|a| !tried.contains(&a.id))
                        .take(16)
                        .collect()
                };
                let mut tasks = Vec::new();
                for a in candidates {
                    me.enrich_tried.lock().insert(a.id.clone());
                    let me2 = me.clone();
                    tasks.push(tokio::spawn(async move {
                        if let Some(img) = me2.fetch_og_image(&a.url).await {
                            me2.store.set_image(&a.id, img);
                        }
                    }));
                }
                futures::future::join_all(tasks).await;
                let mut tried = me.enrich_tried.lock();
                if tried.len() > 20_000 {
                    tried.clear();
                }
            }
        });
    }

    async fn fetch_og_image(&self, page: &str) -> Option<String> {
        let _p = self.permits.acquire().await.ok()?;
        let mut resp = self.http.get(page).timeout(Duration::from_secs(8)).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        // Only the <head> matters; stop after ~160 KB.
        let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);
        while let Ok(Some(chunk)) = resp.chunk().await {
            buf.extend_from_slice(&chunk);
            if buf.len() > 160 * 1024 || buf.windows(7).any(|w| w.eq_ignore_ascii_case(b"</head>")) {
                break;
            }
        }
        let html = String::from_utf8_lossy(&buf);
        util::og_image(&html).filter(|u| !util::looks_like_pixel(u))
    }
}

fn pick_image(entry: &feed_rs::model::Entry) -> Option<String> {
    let mut best: Option<(u32, String)> = None;
    for m in &entry.media {
        for c in &m.content {
            let Some(u) = c.url.as_ref().map(|u| u.to_string()) else { continue };
            let is_image = c
                .content_type
                .as_ref()
                .map(|t| t.to_string().starts_with("image"))
                .unwrap_or_else(|| {
                    // No MIME type (common with media:content medium="image"): accept anything
                    // that isn't obviously audio/video.
                    let l = u.to_lowercase();
                    ![".mp4", ".mp3", ".m3u8", ".m4a", ".webm", ".mov", "youtube.com"]
                        .iter()
                        .any(|x| l.contains(x))
                });
            if is_image && !util::looks_like_pixel(&u) {
                let w = c.width.unwrap_or(400);
                if best.as_ref().map_or(true, |(bw, _)| w > *bw) {
                    best = Some((w, u));
                }
            }
        }
        for t in &m.thumbnails {
            let w = t.image.width.unwrap_or(200);
            if best.as_ref().map_or(true, |(bw, _)| w > *bw) && t.image.uri.starts_with("http") {
                best = Some((w, t.image.uri.clone()));
            }
        }
    }
    best.map(|(_, u)| u)
}

fn parse_gdelt_date(s: &str) -> Option<DateTime<Utc>> {
    let n = chrono::NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%SZ").ok()?;
    Some(Utc.from_utc_datetime(&n))
}

fn pretty_source(domain: &str) -> String {
    let d = domain.trim_start_matches("www.");
    let base = d.split('.').next().unwrap_or(d);
    let mut c = base.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => d.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gdelt_dates() {
        let d = parse_gdelt_date("20261111T123000Z").unwrap();
        assert_eq!(d.to_rfc3339(), "2026-11-11T12:30:00+00:00");
    }

    #[test]
    fn source_names() {
        assert_eq!(pretty_source("www.reuters.com"), "Reuters");
    }
}
