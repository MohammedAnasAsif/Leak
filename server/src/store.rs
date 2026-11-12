use crate::model::{Article, LiveEvent, SourceStatus};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;

/// In-memory story index. Reads vastly outnumber writes, so a RwLock over
/// a sorted Vec is both simple and very fast at the sizes we keep (~6k).
pub struct Store {
    inner: RwLock<Inner>,
    pub live: broadcast::Sender<LiveEvent>,
    pub sources: RwLock<HashMap<String, SourceStatus>>,
    max_articles: usize,
    snapshot_path: PathBuf,
    pub started: DateTime<Utc>,
}

#[derive(Default)]
struct Inner {
    /// Newest first.
    articles: Vec<Arc<Article>>,
    by_id: HashMap<String, Arc<Article>>,
    seen_urls: HashSet<String>,
    seen_titles: HashSet<String>,
    /// Detection lags of the most recent stories, seconds.
    lags: VecDeque<i64>,
}

pub struct Query<'a> {
    pub category: Option<&'a str>,
    pub search: Option<&'a str>,
    pub before: Option<DateTime<Utc>>,
    pub since: Option<DateTime<Utc>>,
    pub source: Option<&'a str>,
    pub limit: usize,
}

impl Store {
    pub fn new(max_articles: usize, data_dir: &str) -> Arc<Self> {
        let (live, _) = broadcast::channel(1024);
        let _ = std::fs::create_dir_all(data_dir);
        Arc::new(Self {
            inner: RwLock::new(Inner::default()),
            live,
            sources: RwLock::new(HashMap::new()),
            max_articles,
            snapshot_path: PathBuf::from(data_dir).join("snapshot.json"),
            started: Utc::now(),
        })
    }

    /// Insert if new. Returns true when the story had not been seen before.
    /// `announce` = push to live clients (false for a feed's very first read, which is backlog).
    pub fn insert(&self, a: Article, url_key: String, title_key: String, announce: bool) -> bool {
        let a = Arc::new(a);
        {
            let mut g = self.inner.write();
            if g.seen_urls.contains(&url_key) || g.seen_titles.contains(&title_key) {
                return false;
            }
            g.seen_urls.insert(url_key);
            g.seen_titles.insert(title_key);
            g.by_id.insert(a.id.clone(), a.clone());
            let pos = g.articles.partition_point(|x| x.published > a.published);
            g.articles.insert(pos, a.clone());
            if announce && a.via != "gdelt" {
                g.lags.push_back(a.detection_lag_secs());
                if g.lags.len() > 500 {
                    g.lags.pop_front();
                }
            }
            while g.articles.len() > self.max_articles {
                if let Some(old) = g.articles.pop() {
                    g.by_id.remove(&old.id);
                    // seen_* sets are kept (bounded below) so evicted stories don't re-enter.
                }
            }
            if g.seen_urls.len() > self.max_articles * 4 {
                g.seen_urls.clear();
                g.seen_titles.clear();
                let keep: Vec<(String, String)> = g
                    .articles
                    .iter()
                    .map(|x| (crate::util::canonical_url(&x.url), crate::util::title_key(&x.title)))
                    .collect();
                for (u, t) in keep {
                    g.seen_urls.insert(u);
                    g.seen_titles.insert(t);
                }
            }
        }
        if announce {
            let _ = self.live.send(LiveEvent::New(a));
        }
        true
    }

    /// Replace an article's image after enrichment (no re-broadcast of the story itself).
    pub fn set_image(&self, id: &str, image: String) -> Option<Arc<Article>> {
        self.modify(id, |a| a.image = Some(image))
    }

    /// Attach the savage headline and push it to live clients.
    pub fn set_savage(&self, id: &str, headline: String) -> Option<Arc<Article>> {
        self.modify(id, |a| a.savage_title = Some(headline))
    }

    /// Newest stories (within `max_age_hours`) that have no savage headline yet.
    pub fn needing_savage(&self, max_age_hours: i64, limit: usize) -> Vec<Arc<Article>> {
        let cutoff = Utc::now() - chrono::Duration::hours(max_age_hours);
        self.inner
            .read()
            .articles
            .iter()
            .take_while(|a| a.published > cutoff)
            .filter(|a| a.savage_title.is_none())
            .take(limit)
            .cloned()
            .collect()
    }

    /// Apply a change to one stored story and broadcast it as an `update`.
    fn modify(&self, id: &str, f: impl FnOnce(&mut Article)) -> Option<Arc<Article>> {
        let mut g = self.inner.write();
        let current = g.by_id.get(id)?.clone();
        let mut updated = (*current).clone();
        f(&mut updated);
        let updated = Arc::new(updated);
        g.by_id.insert(id.to_string(), updated.clone());
        if let Some(slot) = g.articles.iter_mut().find(|x| x.id == id) {
            *slot = updated.clone();
        }
        let _ = self.live.send(LiveEvent::Update(updated.clone()));
        Some(updated)
    }

    pub fn has_url(&self, url_key: &str) -> bool {
        self.inner.read().seen_urls.contains(url_key)
    }

    pub fn get(&self, id: &str) -> Option<Arc<Article>> {
        self.inner.read().by_id.get(id).cloned()
    }

    pub fn query(&self, q: &Query) -> Vec<Arc<Article>> {
        let needle = q.search.map(|s| s.to_lowercase());
        let g = self.inner.read();
        g.articles
            .iter()
            .filter(|a| q.category.map_or(true, |c| a.category == c))
            .filter(|a| q.source.map_or(true, |s| a.source_domain == s))
            .filter(|a| q.before.map_or(true, |b| a.published < b))
            .filter(|a| q.since.map_or(true, |s| a.detected > s))
            .filter(|a| {
                needle.as_deref().map_or(true, |n| {
                    a.title.to_lowercase().contains(n)
                        || a.summary.to_lowercase().contains(n)
                        || a.savage_title.as_deref().is_some_and(|s| s.to_lowercase().contains(n))
                })
            })
            .take(q.limit)
            .cloned()
            .collect()
    }

    pub fn counts(&self) -> HashMap<String, usize> {
        let mut m = HashMap::new();
        for a in self.inner.read().articles.iter() {
            *m.entry(a.category.clone()).or_insert(0) += 1;
        }
        m
    }

    pub fn total(&self) -> usize {
        self.inner.read().articles.len()
    }

    /// Median seconds from publication to detection, over recent stories.
    pub fn median_lag(&self) -> Option<i64> {
        let g = self.inner.read();
        if g.lags.is_empty() {
            return None;
        }
        let mut v: Vec<i64> = g.lags.iter().copied().collect();
        v.sort_unstable();
        Some(v[v.len() / 2])
    }

    pub fn articles_without_image(&self, limit: usize) -> Vec<Arc<Article>> {
        self.inner
            .read()
            .articles
            .iter()
            .filter(|a| a.image.is_none())
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn update_source(&self, id: &str, f: impl FnOnce(&mut SourceStatus)) {
        if let Some(s) = self.sources.write().get_mut(id) {
            f(s);
        }
    }

    pub fn save_snapshot(&self) {
        let list: Vec<Arc<Article>> = self.inner.read().articles.clone();
        let tmp = self.snapshot_path.with_extension("tmp");
        match serde_json::to_vec(&list) {
            Ok(bytes) => {
                if std::fs::write(&tmp, bytes).is_ok() {
                    let _ = std::fs::rename(&tmp, &self.snapshot_path);
                }
            }
            Err(e) => tracing::warn!("snapshot encode failed: {e}"),
        }
    }

    /// Reload stories from the last run so a restart doesn't show an empty page.
    pub fn load_snapshot(&self) -> usize {
        let Ok(bytes) = std::fs::read(&self.snapshot_path) else { return 0 };
        let Ok(list) = serde_json::from_slice::<Vec<Article>>(&bytes) else { return 0 };
        let cutoff = Utc::now() - chrono::Duration::hours(48);
        let mut g = self.inner.write();
        for a in list.into_iter().filter(|a| a.published > cutoff) {
            g.seen_urls.insert(crate::util::canonical_url(&a.url));
            g.seen_titles.insert(crate::util::title_key(&a.title));
            let a = Arc::new(a);
            g.by_id.insert(a.id.clone(), a.clone());
            g.articles.push(a);
        }
        g.articles.sort_by(|a, b| b.published.cmp(&a.published));
        g.articles.len()
    }
}
