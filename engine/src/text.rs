//! Small, dependency-light text utilities: HTML stripping, tokenizing, slugs.

use std::collections::HashSet;
use std::sync::OnceLock;

/// Remove tags and decode entities, collapse whitespace.
pub fn clean_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    let mut skip_until: Option<&str> = None;
    let lower = input.to_ascii_lowercase();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if let Some(end) = skip_until {
            if lower[i..].starts_with(end) {
                i += end.len();
                skip_until = None;
            } else {
                i += 1;
            }
            continue;
        }
        let c = bytes[i];
        if c == b'<' {
            if lower[i..].starts_with("<script") {
                skip_until = Some("</script>");
                continue;
            }
            if lower[i..].starts_with("<style") {
                skip_until = Some("</style>");
                continue;
            }
            // Block-ish tags become spaces so words don't glue together.
            out.push(' ');
            in_tag = true;
            i += 1;
            continue;
        }
        if c == b'>' && in_tag {
            in_tag = false;
            i += 1;
            continue;
        }
        if !in_tag {
            // Push the full UTF-8 char.
            let ch_len = utf8_len(c);
            out.push_str(&input[i..i + ch_len]);
            i += ch_len;
        } else {
            i += 1;
        }
    }
    let decoded = html_escape::decode_html_entities(&out).to_string();
    collapse_ws(&decoded)
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

pub fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Truncate at a sentence or word boundary, adding an ellipsis when cut.
pub fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let cut: String = s.chars().take(max_chars).collect();
    // Prefer ending on a full sentence if one finishes in the back half.
    if let Some(pos) = cut.rfind(['.', '!', '?']) {
        if pos > max_chars / 2 {
            return cut[..=pos].to_string();
        }
    }
    match cut.rfind(' ') {
        Some(pos) => format!(
            "{}…",
            cut[..pos].trim_end_matches([',', ';', ':', '-', '–'])
        ),
        None => format!("{cut}…"),
    }
}

pub fn slugify(s: &str, max_words: usize) -> String {
    let mut words = Vec::new();
    let mut cur = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            cur.push(ch.to_ascii_lowercase());
        } else if !cur.is_empty() {
            words.push(std::mem::take(&mut cur));
        }
        if words.len() >= max_words {
            break;
        }
    }
    if !cur.is_empty() && words.len() < max_words {
        words.push(cur);
    }
    let slug = words.join("-");
    if slug.is_empty() {
        "story".into()
    } else {
        slug
    }
}

fn stopwords() -> &'static HashSet<&'static str> {
    static SW: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SW.get_or_init(|| {
        [
            "a", "an", "the", "and", "or", "but", "if", "of", "at", "by", "for", "with", "about",
            "against", "between", "into", "through", "during", "before", "after", "above", "below",
            "to", "from", "up", "down", "in", "out", "on", "off", "over", "under", "again",
            "further", "then", "once", "here", "there", "when", "where", "why", "how", "all",
            "any", "both", "each", "few", "more", "most", "other", "some", "such", "no", "nor",
            "not", "only", "own", "same", "so", "than", "too", "very", "can", "will", "just",
            "should", "now", "is", "are", "was", "were", "be", "been", "being", "have", "has",
            "had", "having", "do", "does", "did", "doing", "it", "its", "this", "that", "these",
            "those", "he", "she", "they", "them", "his", "her", "their", "we", "our", "you",
            "your", "i", "me", "my", "who", "whom", "which", "what", "as", "would", "could", "may",
            "might", "must", "says", "said", "say", "new", "news", "live", "latest", "update",
            "updates", "breaking", "watch", "video", "photos", "report", "reports", "amid",
            "after", "over", "via", "us", "also", "first", "one", "two", "year", "years", "day",
            "days", "week", "weeks", "time", "get", "gets", "make", "makes", "set", "sets", "back",
            "still", "yet", "s", "t", "vs", "how", "why", "what", "here's", "heres", "today",
        ]
        .into_iter()
        .collect()
    })
}

/// A few high-value synonyms so paraphrased headlines meet in the middle.
fn canonical(word: &str) -> &str {
    match word {
        "quake" | "earthquakes" | "tremor" => "earthquake",
        "polls" | "poll" | "ballot" => "election",
        "pm" => "prime-minister",
        "govt" | "gov" => "government",
        "killed" | "kills" | "kill" | "dead" | "deaths" | "die" | "dies" | "died" => "death",
        "us" | "usa" => "united-states",
        "uk" | "britain" | "british" => "united-kingdom",
        "rbi" => "reserve-bank-india",
        "fed" => "federal-reserve",
        "ai" => "artificial-intelligence",
        "ceasefire" | "truce" => "ceasefire",
        _ => word,
    }
}

fn stem(word: &str) -> String {
    let w = word;
    if w.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return w.to_string();
    }
    let n = w.len();
    if n > 6 && w.ends_with("ing") {
        return w[..n - 3].to_string();
    }
    if n > 5 && w.ends_with("ed") && !w.ends_with("eed") {
        return w[..n - 2].to_string();
    }
    if n > 4 && w.ends_with("ies") {
        return format!("{}y", &w[..n - 3]);
    }
    if n > 4 && w.ends_with('s') && !w.ends_with("ss") && !w.ends_with("us") && !w.ends_with("is") {
        return w[..n - 1].to_string();
    }
    w.to_string()
}

/// Lowercased, stemmed content tokens. Keeps decimals like "7.1" intact.
pub fn tokens(s: &str) -> Vec<String> {
    let lower = s.to_lowercase().replace(['’', '‘'], "'");
    let chars: Vec<char> = lower.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    for (i, &ch) in chars.iter().enumerate() {
        let keep_dot = ch == '.'
            && !cur.is_empty()
            && cur.chars().last().is_some_and(|c| c.is_ascii_digit())
            && chars.get(i + 1).is_some_and(|c| c.is_ascii_digit());
        if ch.is_alphanumeric() || keep_dot {
            cur.push(ch);
        } else if ch == '\'' && !cur.is_empty() {
            // drop possessive 's
            if chars.get(i + 1) == Some(&'s')
                && !chars.get(i + 2).is_some_and(|c| c.is_alphanumeric())
            {
                push_token(&mut out, std::mem::take(&mut cur));
            }
        } else if !cur.is_empty() {
            push_token(&mut out, std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        push_token(&mut out, cur);
    }
    out
}

fn push_token(out: &mut Vec<String>, word: String) {
    if word.len() < 2 && !word.chars().all(|c| c.is_ascii_digit()) {
        return;
    }
    if stopwords().contains(word.as_str()) {
        return;
    }
    let canon = canonical(&word);
    let t = stem(canon);
    if t.len() < 2 && !t.chars().all(|c| c.is_ascii_digit()) {
        return;
    }
    out.push(t);
}

/// Phrases from section keyword lists can be multi-word; match on word boundaries.
pub fn contains_phrase(haystack_lower: &str, phrase: &str) -> bool {
    let mut start = 0;
    while let Some(pos) = haystack_lower[start..].find(phrase) {
        let abs = start + pos;
        let before_ok = abs == 0
            || !haystack_lower[..abs]
                .chars()
                .last()
                .is_some_and(|c| c.is_alphanumeric());
        let end = abs + phrase.len();
        let after_ok = end >= haystack_lower.len()
            || !haystack_lower[end..]
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric());
        if before_ok && after_ok {
            return true;
        }
        start = abs + phrase.len().max(1);
        if start >= haystack_lower.len() {
            break;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_html_and_entities() {
        let s = clean_html(
            "<p>Hello&nbsp;<b>world</b> &amp; friends</p><script>x()</script><img src=x>",
        );
        assert_eq!(s, "Hello world & friends");
    }

    #[test]
    fn keeps_unicode() {
        assert_eq!(clean_html("<p>São Paulo — café</p>"), "São Paulo — café");
    }

    #[test]
    fn truncates_on_boundaries() {
        let t = truncate(
            "The first sentence ends here. Then a much longer second sentence follows on.",
            50,
        );
        assert_eq!(t, "The first sentence ends here.");
        let t = truncate("one two three four five six seven", 15);
        assert_eq!(t, "one two three…");
    }

    #[test]
    fn tokenizes_and_normalizes() {
        let t = tokens("Japan's quake: 7.1 magnitude strikes, tsunami warnings issued");
        assert!(t.contains(&"japan".to_string()));
        assert!(t.contains(&"earthquake".to_string()));
        assert!(t.contains(&"7.1".to_string()));
        assert!(t.contains(&"warning".to_string()));
        assert!(!t.contains(&"s".to_string()));
    }

    #[test]
    fn slug() {
        assert_eq!(
            slugify("Japan hit by 7.1 quake — tsunami warning!", 6),
            "japan-hit-by-7-1-quake"
        );
    }

    #[test]
    fn phrase_boundaries() {
        assert!(contains_phrase(
            "the central bank raised rates",
            "central bank"
        ));
        assert!(!contains_phrase("said the airline", "ai"));
        assert!(contains_phrase("ai models are", "ai"));
    }
}
