use once_cell::sync::Lazy;
use regex::Regex;
use std::hash::{Hash, Hasher};

/// 16-hex-char stable hash (FNV-1a 64), used for ids.
pub fn short_hash(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

static TRACKING_PARAMS: &[&str] = &[
    "utm_source", "utm_medium", "utm_campaign", "utm_term", "utm_content", "utm_name",
    "cmpid", "CMP", "ocid", "ref", "rss", "fbclid", "gclid", "mc_cid", "mc_eid", "at_medium",
    "at_campaign", "src", "taid", "ito",
];

/// Canonical form of a story URL so the same story from two feeds dedupes.
pub fn canonical_url(raw: &str) -> String {
    let Ok(mut u) = url::Url::parse(raw.trim()) else {
        return raw.trim().to_string();
    };
    u.set_fragment(None);
    let kept: Vec<(String, String)> = u
        .query_pairs()
        .filter(|(k, _)| !TRACKING_PARAMS.iter().any(|t| t.eq_ignore_ascii_case(k)))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    if kept.is_empty() {
        u.set_query(None);
    } else {
        u.query_pairs_mut().clear().extend_pairs(kept);
    }
    let _ = u.set_scheme("https");
    let host = u.host_str().unwrap_or("").trim_start_matches("www.").to_string();
    let path = u.path().trim_end_matches('/').to_string();
    match u.query() {
        Some(q) => format!("{host}{path}?{q}"),
        None => format!("{host}{path}"),
    }
}

/// Normalised title key, catches the same wire story syndicated under different URLs.
pub fn title_key(title: &str) -> String {
    let mut words: Vec<String> = title
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
        .map(str::to_string)
        .collect();
    words.truncate(12);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    words.hash(&mut h);
    format!("{:x}", h.finish())
}

pub fn domain_of(raw: &str) -> String {
    url::Url::parse(raw)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string()))
        .unwrap_or_default()
}

static TAG: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)<[^>]*>").unwrap());
static SCRIPTS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<(script|style)[^>]*>.*?</(script|style)>").unwrap());
static SPACES: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());
static IMG: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)<img[^>]+src\s*=\s*["']([^"']+)["']"#).unwrap());

/// HTML → clean single-paragraph text.
pub fn strip_html(s: &str) -> String {
    let no_scripts = SCRIPTS.replace_all(s, " ");
    let no_tags = TAG.replace_all(&no_scripts, " ");
    let decoded = html_escape::decode_html_entities(&no_tags);
    SPACES.replace_all(decoded.trim(), " ").to_string()
}

/// Trim to at most `max` chars on a word boundary, adding an ellipsis.
pub fn truncate_words(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    let cut = match cut.rfind(' ') {
        Some(i) if i > max / 2 => &cut[..i],
        _ => &cut[..],
    };
    format!("{}…", cut.trim_end_matches(|c: char| c.is_ascii_punctuation()))
}

pub fn first_img(html: &str) -> Option<String> {
    IMG.captures(html)
        .map(|c| c[1].to_string())
        .filter(|u| u.starts_with("http") && !looks_like_pixel(u))
}

pub fn looks_like_pixel(u: &str) -> bool {
    let l = u.to_lowercase();
    l.contains("pixel") || l.contains("tracking") || l.contains("feedburner") || l.ends_with(".gif")
        || l.contains("1x1")
}

/// Rough reading-time estimate for the full story, from whatever text the feed gave us.
pub fn read_minutes(text_words: usize) -> u32 {
    // Feed summaries are a fraction of the full article; most news pieces run 500-1200 words.
    let estimated_full = if text_words > 300 { text_words } else { 600 + text_words * 2 };
    ((estimated_full as f32 / 230.0).round() as u32).clamp(1, 20)
}

static OG_IMAGE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?is)<meta[^>]+(?:property|name)\s*=\s*["'](?:og:image|twitter:image)(?::src)?["'][^>]*>"#)
        .unwrap()
});
static CONTENT_ATTR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)content\s*=\s*["']([^"']+)["']"#).unwrap());

pub fn og_image(html: &str) -> Option<String> {
    let tag = OG_IMAGE.find(html)?.as_str();
    let url = CONTENT_ATTR.captures(tag)?[1].to_string();
    let url = html_escape::decode_html_entities(&url).to_string();
    url.starts_with("http").then_some(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_strips_tracking() {
        assert_eq!(
            canonical_url("http://www.bbc.co.uk/news/world-1?at_medium=RSS&utm_source=x#top"),
            "bbc.co.uk/news/world-1"
        );
        assert_eq!(canonical_url("https://ex.com/a/?id=4&utm_term=z"), "ex.com/a?id=4");
    }

    #[test]
    fn strips_html() {
        assert_eq!(strip_html("<p>Hello&nbsp;<b>world</b> &amp; co</p>"), "Hello world & co");
    }

    #[test]
    fn truncates() {
        let t = truncate_words("one two three four five six", 14);
        assert!(t.ends_with('…') && t.len() < 20, "{t}");
    }

    #[test]
    fn finds_og_image() {
        let h = r#"<head><meta property="og:image" content="https://x.com/a.jpg?w=1&amp;h=2"></head>"#;
        assert_eq!(og_image(h).as_deref(), Some("https://x.com/a.jpg?w=1&h=2"));
    }
}
