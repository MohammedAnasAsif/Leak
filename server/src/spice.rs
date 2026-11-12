//! "Savage" headlines: an LLM rewrites each headline into a sharper, sarcastic
//! version. Runs fully in the background, so it never slows news down:
//! the story is pushed to clients instantly with its original headline, and the
//! savage version follows a second or two later as a live `update` event.
//!
//! Guard rails:
//!  * The original headline is always kept and shown on the article page.
//!  * Tragedies (deaths, disasters, violence, abuse, serious illness) are never
//!    rewritten: a keyword filter skips them before any API call, and the model
//!    is told to return null for anything similar the filter missed.
//!  * The model may not add facts, names or claims that are not in the story.
//!  * A daily cap bounds API spend.
//!
//! Works with Anthropic's API or any OpenAI-compatible chat API (OpenAI, Groq,
//! OpenRouter, Google Gemini's OpenAI endpoint, a local Ollama, ...).

use crate::config::LlmConfig;
use crate::model::{Article, LiveEvent};
use crate::store::Store;
use chrono::{Datelike, Utc};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Stories per API call. Batching keeps cost and rate-limit pressure low.
const BATCH: usize = 12;
const MAX_LEN: usize = 110;

static SENSITIVE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)\b(kill(s|ed|ing)?|dead|deaths?|dies|died|dying|fatal(ly|ities)?|murder(s|ed)?|homicide|shooting|gunman|stabb(ed|ing)|massacre|genocide|terror(ism|ist)?|bomb(s|ed|ing)?|suicide|rape[ds]?|sexual (assault|abuse)|abuse[ds]?|child(ren)? (missing|abducted)|hostages?|casualt(y|ies)|victims?|injured|wounded|funeral|mourn(s|ing)?|earthquake|tsunami|flood(s|ing)?|wildfire|hurricane|cyclone|typhoon|tornado|landslide|famine|epidemic|outbreak|cancer|overdose|war crimes?|air ?strikes?|missile strike|refugees?|evacuat(e|ed|ion))\b",
    )
    .unwrap()
});

/// True when a story is about suffering and must keep its original headline.
pub fn is_sensitive(a: &Article) -> bool {
    SENSITIVE.is_match(&a.title) || SENSITIVE.is_match(&a.summary)
}

const SYSTEM_PROMPT: &str = r#"You write headlines for "Leak", a news app with a savage, witty voice: think a sharp-tongued editor with a dry sense of humour.

For each story you get, rewrite the headline so it is savage: punchy, sarcastic, irreverent, the kind of line people screenshot. Rules:
- Stay 100% faithful to the facts given. Never add numbers, names, quotes, motives or claims that are not in the original headline or summary. Never imply someone committed a crime unless the original says so.
- Roast the situation, the irony, the powerful, the absurd. Never mock victims, ordinary people's suffering, or anyone for their race, religion, gender, sexuality, disability or nationality. No slurs, no profanity.
- If the story involves death, disaster, violence, abuse, serious illness or other human suffering, do not rewrite it: return null for that story.
- At most 90 characters. Plain text, no emoji, no hashtags, no surrounding quotes, no "BREAKING".
- English, even if the original is in another language.

Reply with only JSON in this exact shape, one entry per story, using the story numbers you were given:
{"headlines":[{"n":1,"h":"..."},{"n":2,"h":null}]}"#;

pub struct Spice {
    cfg: LlmConfig,
    store: Arc<Store>,
    http: reqwest::Client,
    /// Ids already sent (or skipped) so they are never paid for twice.
    tried: Mutex<HashSet<String>>,
    /// (day-of-year, count) for the daily cap.
    used_today: Mutex<(u32, u32)>,
    /// Per model: rate-limited or unavailable until this moment.
    cooldown: Mutex<Vec<Option<Instant>>>,
}

/// Why a call failed: wait and try another model, or a real error.
enum CallErr {
    Cooldown(Duration, String),
    Other(anyhow::Error),
}

impl<E: Into<anyhow::Error>> From<E> for CallErr {
    fn from(e: E) -> Self {
        CallErr::Other(e.into())
    }
}

impl Spice {
    pub fn new(cfg: LlmConfig, store: Arc<Store>) -> Arc<Self> {
        let cfg_models_len = cfg.models.len();
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(45))
            .user_agent("LeakNews/0.1 (+https://github.com/MohammedAnasAsif/Leak)")
            .build()
            .expect("http client");
        Arc::new(Self {
            cfg,
            store,
            http,
            tried: Mutex::new(HashSet::new()),
            used_today: Mutex::new((0, 0)),
            cooldown: Mutex::new(vec![None; cfg_models_len]),
        })
    }

    pub fn spawn(self: &Arc<Self>) {
        tracing::info!(provider = %self.cfg.provider, models = %self.cfg.models.join(", "), daily_limit = self.cfg.daily_limit, "savage headlines on");
        let me = self.clone();
        tokio::spawn(async move {
            let mut live = me.store.live.subscribe();
            let started = std::time::Instant::now();
            loop {
                let did = me.run_batch().await;
                if did > 0 {
                    // More may be queued (startup backlog): keep going, gently.
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    continue;
                }
                // Idle: wake on the next new story, or sweep periodically (often
                // right after startup, while feeds deliver their first backlog).
                let sweep = if started.elapsed() < Duration::from_secs(120) { 3 } else { 20 };
                let _ = tokio::time::timeout(Duration::from_secs(sweep), async {
                    loop {
                        match live.recv().await {
                            Ok(LiveEvent::New(_)) => break,
                            Ok(_) => continue,
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => break,
                            Err(_) => {
                                tokio::time::sleep(Duration::from_secs(5)).await;
                                break;
                            }
                        }
                    }
                })
                .await;
                // Let a burst of simultaneous stories land so they share one call.
                tokio::time::sleep(Duration::from_millis(700)).await;
            }
        });
    }

    fn budget_left(&self) -> u32 {
        let today = Utc::now().ordinal();
        let mut u = self.used_today.lock();
        if u.0 != today {
            *u = (today, 0);
        }
        self.cfg.daily_limit.saturating_sub(u.1)
    }

    /// First model (in preference order) that is not cooling down.
    fn pick_model(&self) -> Option<(usize, String)> {
        let now = Instant::now();
        let cd = self.cooldown.lock();
        self.cfg
            .models
            .iter()
            .enumerate()
            .find(|(i, _)| cd[*i].map_or(true, |t| t <= now))
            .map(|(i, m)| (i, m.clone()))
    }

    fn cool(&self, idx: usize, d: Duration) {
        self.cooldown.lock()[idx] = Some(Instant::now() + d);
    }

    /// Put a batch back so it is retried later.
    fn requeue(&self, batch: &[Arc<Article>]) {
        {
            let mut tried = self.tried.lock();
            for a in batch {
                tried.remove(&a.id);
            }
        }
        let mut used = self.used_today.lock();
        used.1 = used.1.saturating_sub(batch.len() as u32);
    }

    /// Rewrites up to one batch. Returns how many stories were attempted.
    async fn run_batch(&self) -> usize {
        let left = self.budget_left() as usize;
        if left == 0 || self.pick_model().is_none() {
            return 0;
        }
        let batch: Vec<Arc<Article>> = {
            let mut tried = self.tried.lock();
            let candidates = self.store.needing_savage(self.cfg.max_age_hours, 400);
            let mut out = Vec::new();
            for a in candidates {
                if tried.contains(&a.id) {
                    continue;
                }
                tried.insert(a.id.clone());
                if is_sensitive(&a) {
                    continue;
                }
                out.push(a);
                if out.len() >= BATCH.min(left) {
                    break;
                }
            }
            if tried.len() > 50_000 {
                tried.clear();
            }
            out
        };
        if batch.is_empty() {
            return 0;
        }
        self.used_today.lock().1 += batch.len() as u32;

        let mut user = String::from("Stories:\n");
        for (i, a) in batch.iter().enumerate() {
            let summary: String = a.summary.chars().take(280).collect();
            user.push_str(&format!(
                "{}. [{}] {}\n   Summary: {}\n",
                i + 1,
                a.category,
                a.title,
                if summary.is_empty() { "(none)" } else { &summary }
            ));
        }

        // Free tiers run out: on a rate limit, rest that model and use the next.
        let text = loop {
            let Some((idx, model)) = self.pick_model() else {
                tracing::info!("all savage-headline models are resting (free-tier limits); stories keep their original headlines meanwhile");
                self.requeue(&batch);
                return 0;
            };
            match self.complete(&model, &user).await {
                Ok(t) => break t,
                Err(CallErr::Cooldown(d, why)) => {
                    tracing::warn!(model = %model, rest_secs = d.as_secs(), "savage model resting: {why}");
                    self.cool(idx, d);
                }
                Err(CallErr::Other(e)) => {
                    tracing::warn!(model = %model, "savage headline call failed: {e}");
                    self.requeue(&batch);
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    return batch.len();
                }
            }
        };
        let n = self.apply(&batch, &text);
        tracing::debug!(sent = batch.len(), rewritten = n, "savage batch");
        batch.len()
    }

    fn apply(&self, batch: &[Arc<Article>], text: &str) -> usize {
        #[derive(Deserialize)]
        struct Out {
            headlines: Vec<Item>,
        }
        #[derive(Deserialize)]
        struct Item {
            n: usize,
            h: Option<String>,
        }
        static THINK: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)<think>.*?</think>").unwrap());
        let text = THINK.replace_all(text, "");
        let text = text.as_ref();
        let json = match (text.find('{'), text.rfind('}')) {
            (Some(s), Some(e)) if e > s => &text[s..=e],
            _ => {
                tracing::warn!("savage reply had no JSON");
                return 0;
            }
        };
        let Ok(out) = serde_json::from_str::<Out>(json) else {
            tracing::warn!("savage reply JSON did not parse");
            return 0;
        };
        let mut done = 0;
        for it in out.headlines {
            let (Some(a), Some(h)) = (batch.get(it.n.wrapping_sub(1)), it.h) else { continue };
            if let Some(clean) = clean_headline(&h, &a.title) {
                if self.store.set_savage(&a.id, clean).is_some() {
                    done += 1;
                }
            }
        }
        done
    }

    async fn complete(&self, model: &str, user: &str) -> Result<String, CallErr> {
        let req = if self.cfg.provider == "anthropic" {
            self.http
                .post(format!("{}/v1/messages", self.cfg.base_url))
                .header("x-api-key", &self.cfg.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&serde_json::json!({
                    "model": model,
                    "max_tokens": 1200,
                    "temperature": 0.9,
                    "system": SYSTEM_PROMPT,
                    "messages": [{ "role": "user", "content": user }],
                }))
        } else {
            let mut body = serde_json::json!({
                "model": model,
                "temperature": 0.9,
                "messages": [
                    { "role": "system", "content": SYSTEM_PROMPT },
                    { "role": "user", "content": user },
                ],
            });
            // Reasoning models: keep thinking short, it only costs free quota.
            if model.contains("gpt-oss") {
                body["reasoning_effort"] = "low".into();
            }
            self.http
                .post(format!("{}/chat/completions", self.cfg.base_url))
                .bearer_auth(&self.cfg.api_key)
                .json(&body)
        };
        let res = req.send().await?;
        let status = res.status().as_u16();
        let retry_after = res
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<f64>().ok());
        let v: serde_json::Value = res.json().await.unwrap_or_default();
        let msg = v["error"]["message"]
            .as_str()
            .or_else(|| v["error"].as_str())
            .or_else(|| v[0]["error"]["message"].as_str())
            .unwrap_or("")
            .to_string();
        if let Some(d) = classify(status, retry_after, &msg) {
            return Err(CallErr::Cooldown(d, format!("HTTP {status} {msg}")));
        }
        if !(200..300).contains(&status) {
            return Err(CallErr::Other(anyhow::anyhow!("HTTP {status}: {msg}")));
        }
        let text = if self.cfg.provider == "anthropic" {
            v["content"][0]["text"].as_str()
        } else {
            v["choices"][0]["message"]["content"].as_str()
        };
        Ok(text.unwrap_or_default().to_string())
    }
}

/// Decides whether a failed call means "rest this model for a while".
/// Free tiers answer 429 when a per-minute or per-day quota is used up, and
/// providers retire models (404 / "model not found"), so the next one is tried.
fn classify(status: u16, retry_after: Option<f64>, msg: &str) -> Option<Duration> {
    let m = msg.to_lowercase();
    if status == 429 || status == 503 && m.contains("overload") {
        let daily = ["per day", "daily", "(rpd)", "(tpd)", "requests per day", "tokens per day", "quota"]
            .iter()
            .any(|k| m.contains(k));
        let hinted = retry_after.unwrap_or(60.0).clamp(5.0, 6.0 * 3600.0);
        let secs = if daily { hinted.max(1800.0) } else { hinted };
        return Some(Duration::from_secs_f64(secs));
    }
    let gone = status == 404
        || (status == 400 || status == 410)
            && m.contains("model")
            && ["not found", "does not exist", "decommission", "deprecat", "not supported", "no longer"]
                .iter()
                .any(|k| m.contains(k));
    gone.then(|| Duration::from_secs(6 * 3600))
}

/// Tidies a model headline; rejects empty, over-long or unchanged ones.
pub fn clean_headline(h: &str, original: &str) -> Option<String> {
    let t = h
        .trim()
        .trim_matches(|c: char| c == '"' || c == '\u{201c}' || c == '\u{201d}' || c == '\'')
        .trim();
    let t = t.strip_prefix("BREAKING:").unwrap_or(t).trim();
    if t.is_empty() || t.chars().count() > MAX_LEN || t.contains('\n') {
        return None;
    }
    if t.eq_ignore_ascii_case(original.trim()) {
        return None;
    }
    Some(t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn art(title: &str, summary: &str) -> Article {
        Article {
            id: "x".into(),
            title: title.into(),
            summary: summary.into(),
            url: "https://example.com".into(),
            image: None,
            source: "Test".into(),
            source_domain: "example.com".into(),
            category: "world".into(),
            author: None,
            published: Utc::now(),
            detected: Utc::now(),
            read_minutes: 1,
            via: "poll".into(),
            savage_title: None,
        }
    }

    #[test]
    fn tragedies_are_skipped() {
        assert!(is_sensitive(&art("Earthquake strikes off coast", "")));
        assert!(is_sensitive(&art("Train derails", "At least 12 people were killed.")));
        assert!(is_sensitive(&art("Wildfire forces thousands to evacuate", "")));
        assert!(!is_sensitive(&art("Central bank holds rates again", "Markets shrug.")));
        assert!(!is_sensitive(&art("Chipmaker unveils 2nm laptop processor", "")));
    }

    #[test]
    fn free_tier_limits() {
        // Per-minute limit: rest for the hinted time.
        assert_eq!(classify(429, Some(7.0), "Rate limit reached for requests per minute"), Some(Duration::from_secs(7)));
        // Daily quota: rest at least 30 minutes.
        assert_eq!(classify(429, Some(20.0), "Rate limit reached ... tokens per day (TPD)"), Some(Duration::from_secs(1800)));
        // Retired model: skip it for hours.
        assert_eq!(classify(400, None, "The model `llama-3.3-70b-versatile` has been decommissioned"), Some(Duration::from_secs(21600)));
        assert_eq!(classify(404, None, ""), Some(Duration::from_secs(21600)));
        // Bad key / server errors are real errors, not cooldowns.
        assert_eq!(classify(401, None, "Invalid API Key"), None);
        assert_eq!(classify(500, None, "oops"), None);
    }

    #[test]
    fn cleaning() {
        assert_eq!(clean_headline("  \"Rates frozen, again\" ", "x").as_deref(), Some("Rates frozen, again"));
        assert_eq!(clean_headline("BREAKING: Oops", "x").as_deref(), Some("Oops"));
        assert_eq!(clean_headline("Same", "same"), None);
        assert_eq!(clean_headline("", "x"), None);
        assert_eq!(clean_headline(&"a".repeat(200), "x"), None);
    }
}
