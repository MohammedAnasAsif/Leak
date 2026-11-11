//! Derive everything the templates and JSON API need from the raw state.

use crate::cluster::{cosine, terms, Centroid, Idf, Vector, SIM_THRESHOLD};
use crate::config::{Config, REGIONS};
use crate::model::{Article, State, Story};
use crate::text::{contains_phrase, tokens};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, Serialize)]
pub struct Coverage {
    pub source: String,
    pub source_id: String,
    pub region: String,
    pub country: String,
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub published: String,
    pub clock: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RegionCount {
    pub name: String,
    pub slug: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Related {
    pub title: String,
    pub path: String,
    pub cover: String,
    pub section_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoryView {
    pub id: String,
    pub path: String,
    pub url: String,
    pub title: String,
    pub dek: String,
    pub dek_source: String,
    pub section: String,
    pub section_name: String,
    pub section_color: String,
    /// Site-relative path of the generated cover art.
    pub cover: String,
    /// Outlets in the order they reported the story (for the byline).
    pub reporters: Vec<String>,
    pub created: String,
    pub updated: String,
    pub created_clock: String,
    pub first_source: String,
    pub first_region: String,
    pub source_count: usize,
    pub article_count: usize,
    pub regions: Vec<RegionCount>,
    pub coverage: Vec<Coverage>,
    pub tags: Vec<String>,
    pub related: Vec<Related>,
    pub breaking: bool,
    pub developing: bool,
    pub score: f64,
    #[serde(skip)]
    pub created_at: DateTime<Utc>,
    #[serde(skip)]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WireItem {
    pub title: String,
    pub source: String,
    pub region: String,
    pub section: String,
    pub section_name: String,
    pub story_path: String,
    pub story_sources: usize,
    pub published: String,
    pub clock: String,
}

pub struct Views {
    pub stories: Vec<StoryView>,
    pub wire: Vec<WireItem>,
    pub generated: DateTime<Utc>,
}

pub fn region_slug(r: &str) -> String {
    r.to_ascii_lowercase().replace(' ', "-")
}

pub fn clock(t: &DateTime<Utc>) -> String {
    t.format("%H:%M UTC").to_string()
}

pub fn build(state: &State, cfg: &Config, now: DateTime<Utc>) -> Views {
    let mut idf = Idf::default();
    let tfs: HashMap<&str, Vector> = state
        .articles
        .values()
        .map(|a| (a.id.as_str(), terms(&a.title, &a.snippet)))
        .collect();
    for t in tfs.values() {
        idf.add_doc(t);
    }
    let vecs: HashMap<&str, Vector> = tfs.iter().map(|(k, tf)| (*k, idf.weigh(tf))).collect();

    let mut centroids: HashMap<&str, Centroid> = HashMap::new();
    let mut stories: Vec<StoryView> = Vec::with_capacity(state.stories.len());
    for story in state.stories.values() {
        let members: Vec<&Article> = story
            .articles
            .iter()
            .filter_map(|id| state.articles.get(id))
            .collect();
        if members.is_empty() {
            continue;
        }
        let mut c = Centroid::default();
        for a in &members {
            if let Some(v) = vecs.get(a.id.as_str()) {
                c.add(v);
            }
        }
        stories.push(story_view(story, &members, &c, &vecs, &idf, cfg, now));
        centroids.insert(story.id.as_str(), c);
    }

    add_related(&mut stories, &centroids);
    stories.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| b.updated.cmp(&a.updated))
    });

    let story_map: HashMap<&str, &StoryView> = stories.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut arts: Vec<&Article> = state.articles.values().collect();
    arts.sort_by(|a, b| b.published.cmp(&a.published).then_with(|| a.id.cmp(&b.id)));
    let wire = arts
        .into_iter()
        .filter_map(|a| {
            let s = story_map.get(a.story.as_str())?;
            let src = cfg.source(&a.source)?;
            Some(WireItem {
                title: a.title.clone(),
                source: src.name.clone(),
                region: src.region.clone(),
                section: s.section.clone(),
                section_name: s.section_name.clone(),
                story_path: s.path.clone(),
                story_sources: s.source_count,
                published: a.published.to_rfc3339(),
                clock: clock(&a.published),
            })
        })
        .take(200)
        .collect();

    Views {
        stories,
        wire,
        generated: now,
    }
}

fn story_view(
    story: &Story,
    members: &[&Article],
    centroid: &Centroid,
    vecs: &HashMap<&str, Vector>,
    idf: &Idf,
    cfg: &Config,
    now: DateTime<Utc>,
) -> StoryView {
    let weight_of = |a: &Article| cfg.source(&a.source).map(|s| s.weight).unwrap_or(1.0);

    // Headline: the most representative report from a strong outlet.
    let lead = members
        .iter()
        .max_by(|a, b| {
            let sa = weight_of(a)
                + 0.6
                    * vecs
                        .get(a.id.as_str())
                        .map(|v| cosine(v, &centroid.vec))
                        .unwrap_or(0.0) as f64;
            let sb = weight_of(b)
                + 0.6
                    * vecs
                        .get(b.id.as_str())
                        .map(|v| cosine(v, &centroid.vec))
                        .unwrap_or(0.0) as f64;
            sa.total_cmp(&sb)
                .then_with(|| b.published.cmp(&a.published))
        })
        .copied()
        .expect("non-empty");
    let (dek, dek_src) = if !lead.snippet.is_empty() {
        (lead.snippet.clone(), lead.source.clone())
    } else {
        members
            .iter()
            .filter(|a| !a.snippet.is_empty())
            .max_by(|a, b| weight_of(a).total_cmp(&weight_of(b)))
            .map(|a| (a.snippet.clone(), a.source.clone()))
            .unwrap_or_default()
    };

    let mut by_time: Vec<&Article> = members.to_vec();
    by_time.sort_by(|a, b| a.published.cmp(&b.published).then_with(|| a.id.cmp(&b.id)));
    let first = by_time[0];

    let mut sources = HashSet::new();
    let mut region_counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut coverage = Vec::new();
    for a in &by_time {
        let Some(src) = cfg.source(&a.source) else {
            continue;
        };
        if sources.insert(src.id.as_str()) {
            *region_counts.entry(src.region.as_str()).or_default() += 1;
        }
        coverage.push(Coverage {
            source: src.name.clone(),
            source_id: src.id.clone(),
            region: src.region.clone(),
            country: src.country.clone(),
            title: a.title.clone(),
            url: a.url.clone(),
            snippet: a.snippet.clone(),
            published: a.published.to_rfc3339(),
            clock: clock(&a.published),
        });
    }
    let mut regions: Vec<RegionCount> = region_counts
        .into_iter()
        .map(|(r, c)| RegionCount {
            name: r.to_string(),
            slug: region_slug(r),
            count: c,
        })
        .collect();
    regions.sort_by(|a, b| {
        b.count.cmp(&a.count).then_with(|| {
            let ia = REGIONS.iter().position(|x| *x == a.name).unwrap_or(99);
            let ib = REGIONS.iter().position(|x| *x == b.name).unwrap_or(99);
            ia.cmp(&ib)
        })
    });

    let section = pick_section(members, cfg);
    let section_name = cfg
        .section(&section)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| section.clone());

    let n = sources.len();
    let mins_since_created = (now - story.created).num_minutes();
    let mins_since_updated = (now - story.updated).num_minutes();
    let breaking = n >= 3 && mins_since_created <= 120 && mins_since_updated <= 60;
    let developing = !breaking && n >= 2 && mins_since_updated <= 60;

    let max_weight = members.iter().map(|a| weight_of(a)).fold(0.0, f64::max);
    let coverage_score = (1.0 + n as f64).ln() * 1.6 + (regions.len() as f64 - 1.0).max(0.0) * 0.35;
    let age_h = mins_since_updated.max(0) as f64 / 60.0;
    let mut score = (1.0 + coverage_score) * 0.5f64.powf(age_h / 6.0) * max_weight;
    if breaking {
        score *= 1.3;
    }

    let tags = pick_tags(members, centroid, idf);
    let dek_source = cfg
        .source(&dek_src)
        .map(|s| s.name.clone())
        .unwrap_or_default();
    let path = format!("story/{}-{}.html", story.id, story.slug);

    StoryView {
        id: story.id.clone(),
        url: format!("{}/{}", cfg.site.base_url, path),
        path,
        section_color: cfg
            .section(&section)
            .map(|s| s.color.clone())
            .unwrap_or_else(|| "#2457D6".into()),
        cover: format!("covers/{}.svg", story.id),
        reporters: {
            let mut seen = HashSet::new();
            coverage
                .iter()
                .filter(|c| seen.insert(c.source_id.clone()))
                .map(|c| c.source.clone())
                .collect()
        },
        title: lead.title.clone(),
        dek,
        dek_source,
        section,
        section_name,
        created: story.created.to_rfc3339(),
        updated: story.updated.to_rfc3339(),
        created_clock: clock(&story.created),
        first_source: cfg
            .source(&first.source)
            .map(|s| s.name.clone())
            .unwrap_or_default(),
        first_region: cfg
            .source(&first.source)
            .map(|s| s.region.clone())
            .unwrap_or_default(),
        source_count: n,
        article_count: members.len(),
        regions,
        coverage,
        tags,
        related: vec![],
        breaking,
        developing,
        score: (score * 1000.0).round() / 1000.0,
        created_at: story.created,
        updated_at: story.updated,
    }
}

/// Specialist feeds vote strongly; general feeds are routed by keywords.
fn pick_section(members: &[&Article], cfg: &Config) -> String {
    let mut votes: HashMap<String, f64> = HashMap::new();
    for a in members {
        let Some(src) = cfg.source(&a.source) else {
            continue;
        };
        if src.section != "world" {
            *votes.entry(src.section.clone()).or_default() += 1.0 * src.weight;
        } else if let Some(s) = classify(&a.title, &a.snippet, cfg) {
            *votes.entry(s).or_default() += 0.8 * src.weight;
        } else {
            // General feeds with no topical signal only weakly suggest "World",
            // so one specialist outlet outvotes them.
            *votes.entry("world".into()).or_default() += 0.3 * src.weight;
        }
    }
    votes
        .into_iter()
        .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|(s, _)| s)
        .unwrap_or_else(|| "world".into())
}

pub fn classify(title: &str, snippet: &str, cfg: &Config) -> Option<String> {
    let t = title.to_lowercase();
    let s = snippet.to_lowercase();
    let mut best: Option<(&str, u32)> = None;
    for sec in &cfg.site.sections {
        let mut score = 0;
        for k in &sec.keywords {
            if contains_phrase(&t, k) {
                score += 2;
            }
            if contains_phrase(&s, k) {
                score += 1;
            }
        }
        if score >= 2 && best.is_none_or(|(_, b)| score > b) {
            best = Some((sec.id.as_str(), score));
        }
    }
    best.map(|(id, _)| id.to_string())
}

/// Readable topic tags: the story's strongest words, in the casing outlets used.
fn pick_tags(members: &[&Article], centroid: &Centroid, idf: &Idf) -> Vec<String> {
    let mut surface: HashMap<String, HashMap<String, usize>> = HashMap::new();
    for a in members.iter().take(30) {
        for text in [&a.title, &a.snippet] {
            for word in text.split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '\'')) {
                let word = word.trim_matches(|c: char| c == '\'' || c == '-');
                let w = word.strip_suffix("'s").unwrap_or(word);
                if w.len() < 3 {
                    continue;
                }
                let toks = tokens(w);
                if toks.len() == 1 {
                    *surface
                        .entry(toks[0].clone())
                        .or_default()
                        .entry(w.to_string())
                        .or_default() += 1;
                }
            }
        }
    }
    // Multi-word names ("Port Amara", "Reform Alliance") seen at least twice.
    let mut phrases: HashMap<String, usize> = HashMap::new();
    for a in members.iter().take(30) {
        for text in [&a.title, &a.snippet] {
            let mut run: Vec<&str> = Vec::new();
            let mut flush = |run: &mut Vec<&str>| {
                if (2..=3).contains(&run.len()) {
                    *phrases.entry(run.join(" ")).or_default() += 1;
                }
                run.clear();
            };
            for raw in text.split_whitespace() {
                let w = raw.trim_matches(|c: char| !c.is_alphanumeric());
                let w = w
                    .strip_suffix("'s")
                    .or_else(|| w.strip_suffix("’s"))
                    .unwrap_or(w);
                let cap = w.len() >= 2 && w.chars().next().is_some_and(|c| c.is_uppercase());
                if cap {
                    run.push(w);
                } else {
                    flush(&mut run);
                }
                if raw.ends_with([',', ':', ';', '.', '?', '!']) {
                    flush(&mut run);
                }
            }
            flush(&mut run);
        }
    }
    let mut used: HashSet<String> = HashSet::new();
    let mut tags = Vec::new();
    for (term, _) in centroid.top_terms(12) {
        if tags.len() >= 4 {
            break;
        }
        if used.contains(term)
            || term.chars().any(|c| c.is_ascii_digit())
            || term.len() < 3
            || !idf.is_meaningful(term)
        {
            continue;
        }
        let phrase = phrases
            .iter()
            .filter(|(p, c)| **c >= 2 && tokens(p).iter().any(|t| t == term))
            .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)));
        if let Some((p, _)) = phrase {
            used.extend(tokens(p));
            if !tags.iter().any(|t: &String| t.eq_ignore_ascii_case(p)) {
                tags.push(p.clone());
            }
            continue;
        }
        let Some(forms) = surface.get(term) else {
            continue;
        };
        // Prefer a capitalised form (a name), then the most common one.
        let best = forms
            .iter()
            .max_by_key(|(f, c)| {
                (
                    f.chars().next().is_some_and(|ch| ch.is_uppercase())
                        && !f.chars().all(|ch| ch.is_uppercase() || !ch.is_alphabetic()),
                    **c,
                    std::cmp::Reverse((*f).clone()),
                )
            })
            .map(|(f, _)| f.clone());
        if let Some(b) = best {
            let mut chars = b.chars();
            let b: String = chars
                .next()
                .map(|c| c.to_uppercase().chain(chars).collect())
                .unwrap_or_default();
            if !tags.iter().any(|t: &String| t.eq_ignore_ascii_case(&b)) {
                tags.push(b);
            }
        }
    }
    tags
}

fn add_related(stories: &mut [StoryView], centroids: &HashMap<&str, Centroid>) {
    let mut index: HashMap<&str, Vec<usize>> = HashMap::new();
    let tops: Vec<Vec<&str>> = stories
        .iter()
        .map(|s| {
            centroids
                .get(s.id.as_str())
                .map(|c| {
                    c.top_terms(6)
                        .into_iter()
                        .map(|(t, _)| t.as_str())
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();
    for (i, t) in tops.iter().enumerate() {
        for term in t {
            index.entry(term).or_default().push(i);
        }
    }
    let mut related_all: Vec<Vec<Related>> = Vec::with_capacity(stories.len());
    for (i, s) in stories.iter().enumerate() {
        let Some(ci) = centroids.get(s.id.as_str()) else {
            related_all.push(vec![]);
            continue;
        };
        let mut cands: HashSet<usize> = HashSet::new();
        for term in &tops[i] {
            if let Some(list) = index.get(term) {
                if list.len() > 300 {
                    continue;
                }
                cands.extend(list.iter().copied().filter(|&j| j != i));
            }
        }
        let mut scored: Vec<(usize, f32)> = cands
            .into_iter()
            .filter_map(|j| {
                let cj = centroids.get(stories[j].id.as_str())?;
                let sim = cosine(&ci.vec, &cj.vec);
                (0.12..SIM_THRESHOLD + 0.25)
                    .contains(&sim)
                    .then_some((j, sim))
            })
            .collect();
        scored.sort_by(|a, b| {
            b.1.total_cmp(&a.1)
                .then_with(|| stories[a.0].id.cmp(&stories[b.0].id))
        });
        related_all.push(
            scored
                .into_iter()
                .take(3)
                .map(|(j, _)| Related {
                    title: stories[j].title.clone(),
                    path: stories[j].path.clone(),
                    cover: stories[j].cover.clone(),
                    section_name: stories[j].section_name.clone(),
                })
                .collect(),
        );
    }
    for (s, r) in stories.iter_mut().zip(related_all) {
        s.related = r;
    }
}
