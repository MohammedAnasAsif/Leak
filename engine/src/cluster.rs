//! Story clustering: TF-IDF vectors over headlines (+ a little of the snippet),
//! incremental assignment so story ids and links never change.

use crate::text::tokens;
use std::collections::{HashMap, HashSet};

pub type Vector = HashMap<String, f32>;

/// Join an existing story when full-text cosine similarity reaches this…
pub const SIM_THRESHOLD: f32 = 0.30;
/// …or when headline-only similarity reaches this (snippets dilute paraphrases)…
pub const TITLE_THRESHOLD: f32 = 0.30;
/// …and at least this many meaningful words are shared.
pub const MIN_SHARED: usize = 2;
const SNIPPET_WEIGHT: f32 = 0.35;
const SNIPPET_TOKENS: usize = 40;

/// Headline-only term frequencies. Names (capitalised mid-headline) and numbers
/// are the strongest "same event" signals, so they count extra.
pub fn title_terms(title: &str) -> Vector {
    let mut v = Vector::new();
    for (i, word) in title.split_whitespace().enumerate() {
        let is_name = i > 0 && word.chars().next().is_some_and(|c| c.is_uppercase());
        for t in tokens(word) {
            let w = if t.chars().any(|c| c.is_ascii_digit()) {
                1.6
            } else if is_name {
                1.8
            } else {
                1.0
            };
            *v.entry(t).or_default() += w;
        }
    }
    v
}

/// Tokens that look like names (capitalised mid-headline). Title Case headlines
/// carry no such signal, so they yield none.
pub fn name_tokens(title: &str) -> HashSet<String> {
    let words: Vec<&str> = title.split_whitespace().collect();
    let long: Vec<&&str> = words
        .iter()
        .skip(1)
        .filter(|w| w.chars().filter(|c| c.is_alphabetic()).count() >= 4)
        .collect();
    let caps = long
        .iter()
        .filter(|w| w.chars().next().is_some_and(|c| c.is_uppercase()))
        .count();
    if long.len() >= 3 && caps * 4 > long.len() * 3 {
        return HashSet::new();
    }
    words
        .iter()
        .enumerate()
        .filter(|(i, w)| *i > 0 && w.chars().next().is_some_and(|c| c.is_uppercase()))
        .flat_map(|(_, w)| tokens(w))
        .collect()
}

/// Raw term frequencies for one article.
pub fn terms(title: &str, snippet: &str) -> Vector {
    let mut v = Vector::new();
    for t in tokens(title) {
        *v.entry(t).or_default() += 1.0;
    }
    for t in tokens(snippet).into_iter().take(SNIPPET_TOKENS) {
        *v.entry(t).or_default() += SNIPPET_WEIGHT;
    }
    v
}

#[derive(Default)]
pub struct Idf {
    df: HashMap<String, u32>,
    n: u32,
    /// Every token ever seen capitalised mid-headline: a cheap name lexicon.
    pub names: HashSet<String>,
}

impl Idf {
    pub fn add_title(&mut self, title: &str) {
        self.names.extend(name_tokens(title));
    }
    /// Name tokens for a headline, including a leading name ("India beat…").
    pub fn names_in(&self, title: &str) -> HashSet<String> {
        let mut n = name_tokens(title);
        if let Some(first) = title.split_whitespace().next() {
            n.extend(tokens(first).into_iter().filter(|t| self.names.contains(t)));
        }
        n
    }
    /// Weighted headline vector with names (including a leading one) boosted.
    pub fn title_vector(&self, title: &str) -> Vector {
        let mut tf = title_terms(title);
        if let Some(first) = title.split_whitespace().next() {
            for t in tokens(first) {
                if self.names.contains(&t) {
                    if let Some(w) = tf.get_mut(&t) {
                        *w = w.max(1.8);
                    }
                }
            }
        }
        self.weigh(&tf)
    }
    pub fn add_doc(&mut self, terms: &Vector) {
        self.n += 1;
        for t in terms.keys() {
            *self.df.entry(t.clone()).or_default() += 1;
        }
    }
    pub fn idf(&self, term: &str) -> f32 {
        let df = *self.df.get(term).unwrap_or(&0) as f32;
        ((self.n as f32 + 1.0) / (df + 1.0)).ln() + 1.0
    }
    /// Words this common carry no signal for "same story" decisions.
    pub fn is_meaningful(&self, term: &str) -> bool {
        // Only filler that shows up across a big slice of all coverage is dropped;
        // a huge story's own key words must stay usable for matching.
        if self.n < 50 {
            return true;
        }
        let df = *self.df.get(term).unwrap_or(&0) as f32;
        df / (self.n as f32) < 0.2
    }
    pub fn weigh(&self, tf: &Vector) -> Vector {
        // Sublinear tf, but fractional weights (snippet words) stay fractional.
        let mut v: Vector = tf
            .iter()
            .map(|(t, f)| {
                (
                    t.clone(),
                    if *f >= 1.0 { 1.0 + f.ln() } else { *f } * self.idf(t),
                )
            })
            .collect();
        normalize(&mut v);
        v
    }
}

pub fn normalize(v: &mut Vector) {
    let norm = v.values().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.values_mut() {
            *x /= norm;
        }
    }
}

pub fn cosine(a: &Vector, b: &Vector) -> f32 {
    let (small, big) = if a.len() < b.len() { (a, b) } else { (b, a) };
    small
        .iter()
        .filter_map(|(t, x)| big.get(t).map(|y| x * y))
        .sum()
}

/// Running centroid of a story's member vectors.
#[derive(Default, Clone)]
pub struct Centroid {
    sum: Vector,
    pub vec: Vector,
    pub members: usize,
}

impl Centroid {
    pub fn add(&mut self, v: &Vector) {
        for (t, x) in v {
            *self.sum.entry(t.clone()).or_default() += x;
        }
        self.members += 1;
        self.vec = self.sum.clone();
        normalize(&mut self.vec);
    }
    pub fn top_terms(&self, k: usize) -> Vec<(&String, f32)> {
        let mut v: Vec<(&String, f32)> = self.vec.iter().map(|(t, x)| (t, *x)).collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        v.truncate(k);
        v
    }
}

/// Finds the best matching story for a vector, using an inverted index for speed.
#[derive(Default)]
pub struct Matcher {
    pub centroids: HashMap<String, Centroid>,
    pub title_centroids: HashMap<String, Centroid>,
    member_titles: HashMap<String, Vec<Vector>>,
    index: HashMap<String, Vec<String>>,
}

impl Matcher {
    pub fn add_to(&mut self, story: &str, v: &Vector, title: &Vector) {
        self.title_centroids
            .entry(story.to_string())
            .or_default()
            .add(title);
        let members = self.member_titles.entry(story.to_string()).or_default();
        if members.len() < 40 {
            members.push(title.clone());
        }
        let c = self.centroids.entry(story.to_string()).or_default();
        c.add(v);
        for t in v.keys() {
            let list = self.index.entry(t.clone()).or_default();
            if !list.iter().any(|s| s == story) {
                list.push(story.to_string());
            }
        }
    }

    /// `names` are the new headline's name tokens: overlap made only of two names
    /// ("India", "Australia") is not enough to call it the same event.
    pub fn best(
        &self,
        v: &Vector,
        title: &Vector,
        names: &HashSet<String>,
        idf: &Idf,
    ) -> Option<(String, f32)> {
        let mut candidates: HashMap<&str, usize> = HashMap::new();
        for t in v.keys().filter(|t| idf.is_meaningful(t)) {
            if let Some(list) = self.index.get(t) {
                for s in list {
                    *candidates.entry(s.as_str()).or_default() += 1;
                }
            }
        }
        candidates
            .into_iter()
            .filter(|(_, shared)| *shared >= MIN_SHARED)
            .filter_map(|(s, _)| {
                let c = self.centroids.get(s)?;
                let shared: Vec<&String> = v
                    .keys()
                    .filter(|t| idf.is_meaningful(t) && c.vec.contains_key(*t))
                    .collect();
                if shared.len() < 3 && shared.iter().all(|t| names.contains(*t)) {
                    return None;
                }
                let full = cosine(v, &c.vec);
                let head_avg = self
                    .title_centroids
                    .get(s)
                    .map(|t| cosine(title, &t.vec))
                    .unwrap_or(0.0);
                // Closest single headline, slightly discounted vs the story's average.
                let head_best = self
                    .member_titles
                    .get(s)
                    .map(|m| m.iter().map(|t| cosine(title, t)).fold(0.0, f32::max) * 0.92)
                    .unwrap_or(0.0);
                let head = head_avg.max(head_best);
                let ok = full >= SIM_THRESHOLD || head >= TITLE_THRESHOLD;
                ok.then(|| (s.to_string(), full.max(head)))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus(docs: &[(&str, &str)]) -> (Idf, Vec<Vector>) {
        let tfs: Vec<Vector> = docs.iter().map(|(t, s)| terms(t, s)).collect();
        let mut idf = Idf::default();
        // Background documents so idf behaves like a real day of news.
        let background = [
            "Government announces new budget plan for schools",
            "Football club signs striker in record transfer deal",
            "Film festival opens with drama premiere",
            "Scientists discover new species of frog in rainforest",
            "Oil prices rise as supply concerns grow",
            "Tech company reports quarterly earnings beat",
            "Floods displace thousands after heavy monsoon rain",
            "President meets foreign minister for talks on trade",
            "Talks between India and Pakistan resume in Delhi",
            "Floods hit northern Australia as cyclone nears",
        ];
        for b in background {
            idf.add_doc(&terms(b, ""));
            idf.add_title(b);
        }
        for (tf, (t, _)) in tfs.iter().zip(docs) {
            idf.add_doc(tf);
            idf.add_title(t);
        }
        let vecs = tfs.iter().map(|tf| idf.weigh(tf)).collect();
        (idf, vecs)
    }

    fn same(a: (&str, &str), b: (&str, &str)) -> bool {
        let (idf, v) = corpus(&[a, b]);
        let ta = idf.title_vector(a.0);
        let tb = idf.title_vector(b.0);
        let mut m = Matcher::default();
        m.add_to("s1", &v[0], &ta);
        m.best(&v[1], &tb, &idf.names_in(b.0), &idf).is_some()
    }

    #[test]
    fn paraphrased_headlines_cluster() {
        assert!(same(
            (
                "Magnitude 7.1 earthquake strikes off Japan's coast",
                "A strong earthquake hit off the northeast coast of Japan on Wednesday."
            ),
            (
                "Japan hit by powerful 7.1 quake, tsunami warning issued",
                "Authorities issued a tsunami warning after the quake struck off the coast."
            ),
        ));
        assert!(same(
            (
                "Norland central bank holds interest rates at 4.25%",
                "The central bank kept its key rate unchanged."
            ),
            (
                "Norland keeps rates unchanged at 4.25 per cent as inflation cools",
                ""
            ),
        ));
        assert!(same(
            (
                "Ceasefire agreed between Valdoria and Kestran after talks in Geneva",
                ""
            ),
            (
                "Valdoria, Kestran agree truce following Geneva negotiations",
                ""
            ),
        ));
    }

    #[test]
    fn different_stories_stay_apart() {
        assert!(!same(
            (
                "Japan election: ruling party wins clear majority",
                "Voters returned the governing coalition."
            ),
            (
                "Magnitude 7.1 earthquake strikes off Japan's coast",
                "A strong earthquake hit off the coast."
            ),
        ));
        assert!(!same(
            ("Apple unveils new iPhone with faster chip", ""),
            ("Apple shares fall after weak earnings forecast", "")
        ));
        assert!(!same(
            ("India beat Australia by six wickets in first Test", ""),
            ("India and Australia sign critical minerals agreement", ""),
        ));
    }
}
