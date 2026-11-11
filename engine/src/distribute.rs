//! Free distribution: post the biggest stories to Telegram, Bluesky, Mastodon and ntfy.
//! Each channel is enabled only when its environment variables are set.

use crate::config::Config;
use crate::model::State;
use crate::text::truncate;
use crate::views::{StoryView, Views};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde_json::json;

#[derive(Default)]
pub struct Targets {
    telegram: Option<(String, String)>,
    bluesky: Option<(String, String, String)>,
    mastodon: Option<(String, String)>,
    ntfy: Option<(String, String)>,
}

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.trim().is_empty())
}

impl Targets {
    pub fn from_env() -> Self {
        Targets {
            telegram: env("TELEGRAM_BOT_TOKEN").zip(env("TELEGRAM_CHAT_ID")),
            bluesky: env("BLUESKY_HANDLE")
                .zip(env("BLUESKY_APP_PASSWORD"))
                .map(|(h, p)| {
                    (
                        env("BLUESKY_SERVICE").unwrap_or_else(|| "https://bsky.social".into()),
                        h,
                        p,
                    )
                }),
            mastodon: env("MASTODON_INSTANCE").zip(env("MASTODON_TOKEN")),
            ntfy: env("NTFY_TOPIC").map(|t| {
                (
                    env("NTFY_SERVER").unwrap_or_else(|| "https://ntfy.sh".into()),
                    t,
                )
            }),
        }
    }
    pub fn any(&self) -> bool {
        self.telegram.is_some()
            || self.bluesky.is_some()
            || self.mastodon.is_some()
            || self.ntfy.is_some()
    }
    pub fn names(&self) -> Vec<&'static str> {
        let mut v = vec![];
        if self.telegram.is_some() {
            v.push("telegram");
        }
        if self.bluesky.is_some() {
            v.push("bluesky");
        }
        if self.mastodon.is_some() {
            v.push("mastodon");
        }
        if self.ntfy.is_some() {
            v.push("ntfy");
        }
        v
    }
}

/// Stories worth pushing to followers right now.
pub fn candidates<'a>(
    cfg: &Config,
    state: &State,
    views: &'a Views,
    now: DateTime<Utc>,
) -> Vec<&'a StoryView> {
    let d = &cfg.site.distribution;
    views
        .stories
        .iter()
        .filter(|s| s.source_count >= d.min_sources)
        .filter(|s| (now - s.created_at).num_minutes() <= d.max_story_age_minutes)
        .filter(|s| state.stories.get(&s.id).is_some_and(|st| !st.posted))
        .take(d.max_posts_per_run)
        .collect()
}

pub async fn run(
    cfg: &Config,
    state: &mut State,
    views: &Views,
    now: DateTime<Utc>,
) -> Result<usize> {
    let targets = Targets::from_env();
    if !targets.any() {
        tracing::info!("distribution: no channels configured, skipping");
        return Ok(0);
    }
    let picks: Vec<StoryView> = candidates(cfg, state, views, now)
        .into_iter()
        .cloned()
        .collect();
    if picks.is_empty() {
        return Ok(0);
    }
    let client = crate::fetch::client(cfg)?;
    let mut bsky_session: Option<(String, String)> = None;
    for s in &picks {
        if let Some((token, chat)) = &targets.telegram {
            log_err("telegram", telegram(&client, token, chat, s).await);
        }
        if let Some((service, handle, pw)) = &targets.bluesky {
            if bsky_session.is_none() {
                match bluesky_login(&client, service, handle, pw).await {
                    Ok(sess) => bsky_session = Some(sess),
                    Err(e) => tracing::warn!("bluesky login: {e}"),
                }
            }
            if let Some((jwt, did)) = &bsky_session {
                log_err(
                    "bluesky",
                    bluesky_post(&client, service, jwt, did, s, now).await,
                );
            }
        }
        if let Some((instance, token)) = &targets.mastodon {
            log_err("mastodon", mastodon(&client, instance, token, s).await);
        }
        if let Some((server, topic)) = &targets.ntfy {
            log_err("ntfy", ntfy(&client, server, topic, s).await);
        }
        // Mark as posted even if a channel failed, so a broken channel can't cause repeats.
        if let Some(st) = state.stories.get_mut(&s.id) {
            st.posted = true;
        }
    }
    tracing::info!(
        "distribution: posted {} stories to {:?}",
        picks.len(),
        targets.names()
    );
    Ok(picks.len())
}

fn log_err(channel: &str, r: Result<()>) {
    if let Err(e) = r {
        tracing::warn!("{channel}: {e}");
    }
}

fn coverage_line(s: &StoryView) -> String {
    let regions = s.regions.len();
    format!(
        "Covered by {} outlets across {} region{}.",
        s.source_count,
        regions,
        if regions == 1 { "" } else { "s" }
    )
}

async fn check(resp: reqwest::Response) -> Result<reqwest::Response> {
    if resp.status().is_success() {
        Ok(resp)
    } else {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        Err(anyhow!("HTTP {status}: {}", truncate(&body, 200)))
    }
}

async fn telegram(c: &reqwest::Client, token: &str, chat: &str, s: &StoryView) -> Result<()> {
    let esc = |x: &str| {
        x.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let label = if s.breaking {
        "Breaking"
    } else {
        s.section_name.as_str()
    };
    let text = format!(
        "<b>{}</b>\n\n{}: {}\n<a href=\"{}\">See every outlet's coverage</a>",
        esc(&s.title),
        esc(label),
        esc(&coverage_line(s)),
        esc(&s.url)
    );
    let url = format!("https://api.telegram.org/bot{token}/sendMessage");
    check(
        c.post(url)
            .json(&json!({ "chat_id": chat, "text": text, "parse_mode": "HTML" }))
            .send()
            .await?,
    )
    .await?;
    Ok(())
}

async fn bluesky_login(
    c: &reqwest::Client,
    service: &str,
    handle: &str,
    pw: &str,
) -> Result<(String, String)> {
    let r = check(
        c.post(format!("{service}/xrpc/com.atproto.server.createSession"))
            .json(&json!({ "identifier": handle, "password": pw }))
            .send()
            .await?,
    )
    .await?;
    let v: serde_json::Value = r.json().await?;
    let jwt = v["accessJwt"]
        .as_str()
        .ok_or_else(|| anyhow!("no accessJwt"))?
        .to_string();
    let did = v["did"]
        .as_str()
        .ok_or_else(|| anyhow!("no did"))?
        .to_string();
    Ok((jwt, did))
}

async fn bluesky_post(
    c: &reqwest::Client,
    service: &str,
    jwt: &str,
    did: &str,
    s: &StoryView,
    now: DateTime<Utc>,
) -> Result<()> {
    let prefix = if s.breaking { "Breaking: " } else { "" };
    let text = truncate(&format!("{prefix}{}\n\n{}", s.title, coverage_line(s)), 280);
    let record = json!({
        "$type": "app.bsky.feed.post",
        "text": text,
        "createdAt": now.to_rfc3339(),
        "langs": ["en"],
        "embed": {
            "$type": "app.bsky.embed.external",
            "external": { "uri": s.url, "title": s.title, "description": truncate(&s.dek, 200) }
        }
    });
    check(
        c.post(format!("{service}/xrpc/com.atproto.repo.createRecord"))
            .bearer_auth(jwt)
            .json(&json!({ "repo": did, "collection": "app.bsky.feed.post", "record": record }))
            .send()
            .await?,
    )
    .await?;
    Ok(())
}

async fn mastodon(c: &reqwest::Client, instance: &str, token: &str, s: &StoryView) -> Result<()> {
    let tag = s.section_name.replace(' ', "");
    let status = format!(
        "{}\n\n{} {}\n\n#news #{}",
        s.title,
        coverage_line(s),
        s.url,
        tag
    );
    check(
        c.post(format!(
            "{}/api/v1/statuses",
            instance.trim_end_matches('/')
        ))
        .bearer_auth(token)
        .form(&[("status", status.as_str()), ("visibility", "public")])
        .send()
        .await?,
    )
    .await?;
    Ok(())
}

async fn ntfy(c: &reqwest::Client, server: &str, topic: &str, s: &StoryView) -> Result<()> {
    let tags = if s.breaking {
        vec!["rotating_light"]
    } else {
        vec!["newspaper"]
    };
    check(
        c.post(server.trim_end_matches('/'))
            .json(&json!({
                "topic": topic,
                "title": truncate(&s.title, 120),
                "message": coverage_line(s),
                "click": s.url,
                "tags": tags,
                "priority": if s.breaking { 4 } else { 3 }
            }))
            .send()
            .await?,
    )
    .await?;
    Ok(())
}
