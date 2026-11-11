//! Turn fetched feed items into articles and stories inside the persistent state.

use crate::cluster::{terms, Idf, Matcher};
use crate::config::{short_hash, Config};
use crate::fetch::{FetchResult, RawItem};
use crate::model::{Article, State, Story};
use crate::text::slugify;
use chrono::{DateTime, Duration, Utc};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct IngestStats {
    pub feeds_ok: usize,
    pub feeds_failed: usize,
    pub items_seen: usize,
    pub articles_added: usize,
    pub stories_created: usize,
    pub stories_grown: usize,
    pub new_story_ids: Vec<String>,
}

pub fn ingest(
    state: &mut State,
    cfg: &Config,
    results: Vec<FetchResult>,
    now: DateTime<Utc>,
) -> IngestStats {
    let mut stats = IngestStats::default();
    let mut raw: Vec<RawItem> = Vec::new();
    let mut lags: HashMap<String, Vec<f64>> = HashMap::new();

    for r in results {
        if r.health.last_error.is_some() {
            stats.feeds_failed += 1;
        } else {
            stats.feeds_ok += 1;
        }
        let prev_lag = state.feeds.get(&r.source).and_then(|h| h.lag_minutes);
        let mut health = r.health;
        health.lag_minutes = health.lag_minutes.or(prev_lag);
        state.feeds.insert(r.source.clone(), health);
        stats.items_seen += r.items.len();
        raw.extend(r.items);
    }
    // Forget health for sources removed from the config.
    let live: HashSet<&str> = cfg.sources.iter().map(|s| s.id.as_str()).collect();
    state.feeds.retain(|k, _| live.contains(k.as_str()));

    // New, recent, unique items only.
    let max_age = Duration::hours(cfg.site.max_item_age_hours);
    let mut batch_ids = HashSet::new();
    let mut fresh: Vec<Article> = Vec::new();
    for item in raw {
        let id = short_hash(&item.url, 16);
        if state.articles.contains_key(&id) || !batch_ids.insert(id.clone()) {
            continue;
        }
        let published = item.published.map(|p| p.min(now)).unwrap_or(now);
        if now - published > max_age {
            continue;
        }
        if let Some(p) = item.published {
            let lag = (now - p).num_seconds() as f64 / 60.0;
            if (0.0..=24.0 * 60.0).contains(&lag) {
                lags.entry(item.source.clone()).or_default().push(lag);
            }
        }
        fresh.push(Article {
            id,
            source: item.source,
            url: item.url,
            title: item.title,
            snippet: item.snippet,
            published,
            seen: now,
            story: String::new(),
        });
    }
    // Only the first observation of an item says anything about lag; on the very
    // first run everything is "late" because we just started listening.
    if !state.articles.is_empty() {
        for (src, mut v) in lags {
            v.sort_by(|a, b| a.total_cmp(b));
            let median = v[v.len() / 2];
            if let Some(h) = state.feeds.get_mut(&src) {
                h.lag_minutes = Some(match h.lag_minutes {
                    Some(prev) => prev * 0.7 + median * 0.3,
                    None => median,
                });
            }
        }
    }

    // Vocabulary statistics across everything we know.
    let mut idf = Idf::default();
    let existing_terms: HashMap<&str, _> = state
        .articles
        .values()
        .map(|a| (a.id.as_str(), terms(&a.title, &a.snippet)))
        .collect();
    for t in existing_terms.values() {
        idf.add_doc(t);
    }
    for a in state.articles.values().chain(fresh.iter()) {
        idf.add_title(&a.title);
    }
    let fresh_terms: Vec<_> = fresh.iter().map(|a| terms(&a.title, &a.snippet)).collect();
    for t in &fresh_terms {
        idf.add_doc(t);
    }

    // Centroids for stories still open for new coverage.
    let window = now - Duration::hours(cfg.site.cluster_window_hours);
    let mut matcher = Matcher::default();
    for story in state.stories.values().filter(|s| s.updated >= window) {
        for aid in story.articles.iter().take(40) {
            if let (Some(tf), Some(a)) = (existing_terms.get(aid.as_str()), state.articles.get(aid))
            {
                matcher.add_to(&story.id, &idf.weigh(tf), &idf.title_vector(&a.title));
            }
        }
    }

    // Oldest first, so the earliest report founds the story.
    let mut order: Vec<usize> = (0..fresh.len()).collect();
    order.sort_by_key(|&i| (fresh[i].published, fresh[i].id.clone()));
    let mut grown = HashSet::new();
    for i in order {
        let mut art = fresh[i].clone();
        let v = idf.weigh(&fresh_terms[i]);
        let tv = idf.title_vector(&art.title);
        let story_id = match matcher.best(&v, &tv, &idf.names_in(&art.title), &idf) {
            Some((sid, _)) => {
                let s = state
                    .stories
                    .get_mut(&sid)
                    .expect("matcher only knows existing stories");
                s.articles.push(art.id.clone());
                s.updated = s.updated.max(art.published);
                s.created = s.created.min(art.published);
                if !stats.new_story_ids.contains(&sid) {
                    grown.insert(sid.clone());
                }
                sid
            }
            None => {
                let sid = short_hash(&art.id, 10);
                state.stories.insert(
                    sid.clone(),
                    Story {
                        id: sid.clone(),
                        slug: slugify(&art.title, 8),
                        created: art.published,
                        updated: art.published,
                        articles: vec![art.id.clone()],
                        posted: false,
                    },
                );
                stats.stories_created += 1;
                stats.new_story_ids.push(sid.clone());
                sid
            }
        };
        matcher.add_to(&story_id, &v, &tv);
        art.story = story_id;
        state.articles.insert(art.id.clone(), art);
        stats.articles_added += 1;
    }
    stats.stories_grown = grown.len();
    state.updated = now;
    state.prune(now, cfg.site.retention_hours);
    stats
}
