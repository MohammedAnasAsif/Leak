//! Static site generation: HTML pages, JSON API, RSS feeds, sitemap, PWA files.

use crate::config::Config;
use crate::model::State;
use crate::views::{StoryView, Views, WireItem};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use minijinja::{context, path_loader, Environment, Value};
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct RenderOptions {
    pub web_dir: PathBuf,
    pub out_dir: PathBuf,
    /// Server-sent events endpoint for live mode (serve). None for static hosting.
    pub live_endpoint: Option<String>,
    pub demo: bool,
}

#[derive(Serialize)]
struct SectionNav {
    id: String,
    name: String,
    color: String,
    path: String,
    count: usize,
}

#[derive(Serialize)]
struct SectionBlock<'a> {
    id: &'a str,
    name: &'a str,
    color: &'a str,
    path: String,
    stories: Vec<&'a StoryView>,
}

#[derive(Serialize)]
struct CompactStory<'a> {
    id: &'a str,
    title: &'a str,
    path: &'a str,
    section: &'a str,
    section_name: &'a str,
    sources: usize,
    created: &'a str,
    updated: &'a str,
    breaking: bool,
    developing: bool,
}

impl<'a> From<&'a StoryView> for CompactStory<'a> {
    fn from(s: &'a StoryView) -> Self {
        CompactStory {
            id: &s.id,
            title: &s.title,
            path: &s.path,
            section: &s.section,
            section_name: &s.section_name,
            sources: s.source_count,
            created: &s.created,
            updated: &s.updated,
            breaking: s.breaking,
            developing: s.developing,
        }
    }
}

#[derive(Serialize)]
struct SourceHealthRow {
    name: String,
    section: String,
    region: String,
    ok: bool,
    error: String,
    items: usize,
    lag: String,
    last_ok: String,
}

pub struct Report {
    pub files_written: usize,
    pub stories: usize,
}

pub fn render(cfg: &Config, state: &State, views: &Views, opts: &RenderOptions) -> Result<Report> {
    let out = &opts.out_dir;
    std::fs::create_dir_all(out.join("story"))?;
    std::fs::create_dir_all(out.join("api/sections"))?;
    std::fs::create_dir_all(out.join("feed"))?;
    std::fs::create_dir_all(out.join("assets"))?;

    let mut w = Writer::default();
    let version = copy_static(&opts.web_dir.join("static"), &out.join("assets"), &mut w)?;

    let mut env = Environment::new();
    env.set_loader(path_loader(opts.web_dir.join("templates")));
    env.add_filter("tojson_safe", |v: Value| -> String {
        // JSON for <script type="application/ld+json">: no "</" sequences.
        serde_json::to_string(&v)
            .unwrap_or_default()
            .replace("</", "<\\/")
    });

    let now = views.generated;
    let stories = &views.stories;
    let sections_nav: Vec<SectionNav> = cfg
        .site
        .sections
        .iter()
        .map(|s| SectionNav {
            id: s.id.clone(),
            name: s.name.clone(),
            color: s.color.clone(),
            path: format!("{}.html", s.id),
            count: stories.iter().filter(|st| st.section == s.id).count(),
        })
        .collect();
    let sources_ok = state
        .feeds
        .values()
        .filter(|h| h.last_error.is_none() && h.last_ok.is_some())
        .count();
    let base = context! {
        site => &cfg.site,
        sections => &sections_nav,
        generated => now.to_rfc3339(),
        generated_clock => crate::views::clock(&now),
        generated_date => now.format("%A %-d %B %Y").to_string(),
        v => &version,
        live => &opts.live_endpoint,
        demo => opts.demo,
        source_total => cfg.sources.len(),
        sources_ok => sources_ok,
        story_total => stories.len(),
    };

    // Home
    let lead = stories.first();
    let top: Vec<&StoryView> = stories.iter().skip(1).take(4).collect();
    let shown: HashSet<&str> = stories.iter().take(5).map(|s| s.id.as_str()).collect();
    let blocks: Vec<SectionBlock> = cfg
        .site
        .sections
        .iter()
        .map(|sec| SectionBlock {
            id: &sec.id,
            name: &sec.name,
            color: &sec.color,
            path: format!("{}.html", sec.id),
            stories: stories
                .iter()
                .filter(|s| s.section == sec.id && !shown.contains(s.id.as_str()))
                .take(5)
                .collect(),
        })
        .filter(|b| !b.stories.is_empty())
        .collect();
    let wire: Vec<&WireItem> = views.wire.iter().take(40).collect();
    let breaking: Vec<&StoryView> = stories.iter().filter(|s| s.breaking).take(6).collect();
    let html = env.get_template("index.html")?.render(context! {
        ..base.clone(),
        ..context! {
            root => "",
            page => context! { kind => "home", title => format!("{} — {}", cfg.site.name, cfg.site.tagline), description => &cfg.site.description, canonical => format!("{}/", cfg.site.base_url) },
            lead => lead, top => top, blocks => blocks, wire => wire, breaking => breaking,
        }
    })?;
    w.write(&out.join("index.html"), html)?;

    // Sections
    for sec in &cfg.site.sections {
        let list: Vec<&StoryView> = stories
            .iter()
            .filter(|s| s.section == sec.id)
            .take(60)
            .collect();
        let swire: Vec<&WireItem> = views
            .wire
            .iter()
            .filter(|x| x.section == sec.id)
            .take(25)
            .collect();
        let html = env.get_template("section.html")?.render(context! {
            ..base.clone(),
            ..context! {
                root => "",
                page => context! { kind => "section", section => &sec.id, title => format!("{} news — {}", sec.name, cfg.site.name), description => format!("The latest {} news from outlets around the world, updated every few minutes.", sec.name.to_lowercase()), canonical => format!("{}/{}.html", cfg.site.base_url, sec.id) },
                section => sec, stories => list, wire => swire,
            }
        })?;
        w.write(&out.join(format!("{}.html", sec.id)), html)?;
    }

    // Stories
    let story_tpl = env.get_template("story.html")?;
    let mut keep: HashSet<PathBuf> = HashSet::new();
    for s in stories {
        let more: Vec<&StoryView> = stories
            .iter()
            .filter(|o| o.section == s.section && o.id != s.id)
            .take(5)
            .collect();
        let html = story_tpl.render(context! {
            ..base.clone(),
            ..context! {
                root => "../",
                page => context! { kind => "story", section => &s.section, title => format!("{} — {}", s.title, cfg.site.name), description => if s.dek.is_empty() { format!("{} outlets are covering this story.", s.source_count) } else { s.dek.clone() }, canonical => &s.url },
                story => s, more => more,
            }
        })?;
        let p = out.join(&s.path);
        keep.insert(p.clone());
        w.write(&p, html)?;
    }
    // Remove expired story pages (matters for the long-running server).
    for entry in std::fs::read_dir(out.join("story"))? {
        let p = entry?.path();
        if p.extension().is_some_and(|e| e == "html") && !keep.contains(&p) {
            let _ = std::fs::remove_file(p);
        }
    }

    // Brief: the day's ten most important stories, max three per section.
    let day_ago = now - chrono::Duration::hours(24);
    let mut per_section = std::collections::HashMap::<&str, usize>::new();
    let mut brief_list: Vec<&StoryView> = Vec::new();
    for s in stories
        .iter()
        .filter(|s| s.updated_at >= day_ago && s.source_count >= 2)
    {
        let c = per_section.entry(s.section.as_str()).or_default();
        if *c < 3 {
            *c += 1;
            brief_list.push(s);
        }
        if brief_list.len() == 10 {
            break;
        }
    }
    if brief_list.len() < 10 {
        for s in stories.iter().filter(|s| s.updated_at >= day_ago) {
            if brief_list.len() == 10 {
                break;
            }
            if !brief_list.iter().any(|b| b.id == s.id) {
                brief_list.push(s);
            }
        }
    }
    let html = env.get_template("brief.html")?.render(context! {
        ..base.clone(),
        ..context! {
            root => "",
            page => context! { kind => "brief", title => format!("Today's brief — {}", cfg.site.name), description => "The day's ten most important stories, in about a minute.", canonical => format!("{}/brief.html", cfg.site.base_url) },
            stories => brief_list,
        }
    })?;
    w.write(&out.join("brief.html"), html)?;

    // Simple pages
    let mut health_rows: Vec<SourceHealthRow> = cfg
        .sources
        .iter()
        .map(|src| {
            let h = state.feeds.get(&src.id).cloned().unwrap_or_default();
            SourceHealthRow {
                name: src.name.clone(),
                section: cfg
                    .section(&src.section)
                    .map(|s| s.name.clone())
                    .unwrap_or_default(),
                region: src.region.clone(),
                ok: h.last_error.is_none() && h.last_ok.is_some(),
                error: h.last_error.clone().unwrap_or_default(),
                items: h.last_items,
                lag: h.lag_minutes.map(fmt_minutes).unwrap_or_else(|| "—".into()),
                last_ok: h.last_ok.map(|t| t.to_rfc3339()).unwrap_or_default(),
            }
        })
        .collect();
    health_rows.sort_by(|a, b| a.ok.cmp(&b.ok).then_with(|| a.name.cmp(&b.name)));
    for (tpl, file, title, desc) in [
        (
            "search.html",
            "search.html",
            "Search",
            "Search every story from the last week.",
        ),
        (
            "saved.html",
            "saved.html",
            "Saved stories",
            "Stories you saved on this device.",
        ),
        (
            "about.html",
            "about.html",
            "About",
            "How Leak works, where the news comes from, and how to help.",
        ),
        (
            "404.html",
            "404.html",
            "Page not found",
            "This page is not on Leak.",
        ),
        (
            "offline.html",
            "offline.html",
            "You are offline",
            "Reconnect to load the latest news.",
        ),
    ] {
        // 404 is served at any depth, so it links from the site root instead of relatively.
        let root = if file == "404.html" {
            format!("{}/", cfg.site.base_url)
        } else {
            String::new()
        };
        let html = env.get_template(tpl)?.render(context! {
            ..base.clone(),
            ..context! {
                root => root,
                page => context! { kind => file.trim_end_matches(".html"), title => format!("{title} — {}", cfg.site.name), description => desc, canonical => format!("{}/{}", cfg.site.base_url, file) },
                health => &health_rows,
            }
        })?;
        w.write(&out.join(file), html)?;
    }

    // JSON API
    let compact: Vec<CompactStory> = stories.iter().take(80).map(CompactStory::from).collect();
    w.write(
        &out.join("api/latest.json"),
        serde_json::to_string(&serde_json::json!({
            "generated": now.to_rfc3339(),
            "stories": compact,
            "wire": views.wire.iter().take(60).collect::<Vec<_>>(),
        }))?,
    )?;
    for sec in &cfg.site.sections {
        let list: Vec<CompactStory> = stories
            .iter()
            .filter(|s| s.section == sec.id)
            .take(60)
            .map(CompactStory::from)
            .collect();
        w.write(
            &out.join(format!("api/sections/{}.json", sec.id)),
            serde_json::to_string(&serde_json::json!({ "generated": now.to_rfc3339(), "section": sec.id, "stories": list }))?,
        )?;
    }
    let search: Vec<serde_json::Value> = stories
        .iter()
        .map(|s| {
            serde_json::json!([
                s.title,
                s.path,
                s.section_name,
                s.updated,
                s.tags.join(" "),
                s.source_count,
                s.first_source
            ])
        })
        .collect();
    w.write(
        &out.join("api/search.json"),
        serde_json::to_string(&search)?,
    )?;
    w.write(
        &out.join("api/health.json"),
        serde_json::to_string(
            &serde_json::json!({ "generated": now.to_rfc3339(), "sources": health_rows }),
        )?,
    )?;

    // RSS out
    w.write(
        &out.join("feed.xml"),
        rss(
            cfg,
            &format!("{} — top stories", cfg.site.name),
            &format!("{}/", cfg.site.base_url),
            stories.iter().take(50),
            now,
        ),
    )?;
    for sec in &cfg.site.sections {
        w.write(
            &out.join(format!("feed/{}.xml", sec.id)),
            rss(
                cfg,
                &format!("{} — {}", cfg.site.name, sec.name),
                &format!("{}/{}.html", cfg.site.base_url, sec.id),
                stories.iter().filter(|s| s.section == sec.id).take(40),
                now,
            ),
        )?;
    }

    // Sitemap + robots
    let mut sm = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
    sm.push_str(&format!(
        "<url><loc>{}/</loc><lastmod>{}</lastmod><changefreq>always</changefreq></url>\n",
        cfg.site.base_url,
        now.to_rfc3339()
    ));
    for sec in &cfg.site.sections {
        sm.push_str(&format!(
            "<url><loc>{}/{}.html</loc><lastmod>{}</lastmod></url>\n",
            cfg.site.base_url,
            sec.id,
            now.to_rfc3339()
        ));
    }
    for s in stories {
        sm.push_str(&format!(
            "<url><loc>{}</loc><lastmod>{}</lastmod></url>\n",
            xml_escape(&s.url),
            s.updated
        ));
    }
    sm.push_str("</urlset>\n");
    w.write(&out.join("sitemap.xml"), sm)?;
    w.write(
        &out.join("robots.txt"),
        format!(
            "User-agent: *\nAllow: /\nSitemap: {}/sitemap.xml\n",
            cfg.site.base_url
        ),
    )?;

    // PWA: manifest + service worker live at the root so they control the whole site.
    let manifest = env
        .get_template("manifest.webmanifest")?
        .render(context! { ..base.clone() })?;
    w.write(&out.join("manifest.webmanifest"), manifest)?;
    let sw = std::fs::read_to_string(opts.web_dir.join("sw.js")).context("reading web/sw.js")?;
    w.write(&out.join("sw.js"), sw.replace("__VERSION__", &version))?;
    w.write(&out.join(".nojekyll"), String::new())?;

    Ok(Report {
        files_written: w.count,
        stories: stories.len(),
    })
}

fn fmt_minutes(m: f64) -> String {
    if m < 1.0 {
        "under 1 min".into()
    } else if m < 90.0 {
        format!("{:.0} min", m)
    } else {
        format!("{:.1} h", m / 60.0)
    }
}

#[derive(Default)]
struct Writer {
    count: usize,
}

impl Writer {
    /// Write only when content changed, so mtimes (and caches) stay stable.
    fn write(&mut self, path: &Path, content: impl AsRef<[u8]>) -> Result<()> {
        let content = content.as_ref();
        if let Ok(existing) = std::fs::read(path) {
            if existing == content {
                return Ok(());
            }
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, content).with_context(|| format!("writing {}", path.display()))?;
        self.count += 1;
        Ok(())
    }
}

/// Copy web/static into assets/, returning a content hash used for cache busting.
fn copy_static(from: &Path, to: &Path, w: &mut Writer) -> Result<String> {
    let mut hasher = blake3::Hasher::new();
    let mut files = Vec::new();
    collect_files(from, &mut files)?;
    files.sort();
    for f in files {
        let rel = f.strip_prefix(from)?;
        let bytes = std::fs::read(&f)?;
        hasher.update(rel.to_string_lossy().as_bytes());
        hasher.update(&bytes);
        w.write(&to.join(rel), bytes)?;
    }
    Ok(hasher.finalize().to_hex()[..10].to_string())
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            collect_files(&p, out)?;
        } else {
            out.push(p);
        }
    }
    Ok(())
}

pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn rss<'a>(
    cfg: &Config,
    title: &str,
    link: &str,
    items: impl Iterator<Item = &'a StoryView>,
    now: DateTime<Utc>,
) -> String {
    let mut x = String::new();
    x.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\" xmlns:atom=\"http://www.w3.org/2005/Atom\"><channel>\n");
    x.push_str(&format!(
        "<title>{}</title><link>{}</link><description>{}</description><language>{}</language><lastBuildDate>{}</lastBuildDate><ttl>5</ttl>\n",
        xml_escape(title),
        xml_escape(link),
        xml_escape(&cfg.site.description),
        cfg.site.language,
        now.to_rfc2822()
    ));
    for s in items {
        let mut desc = s.dek.clone();
        if !desc.is_empty() {
            desc.push(' ');
        }
        desc.push_str(&format!(
            "Covered by {} outlet{}.",
            s.source_count,
            if s.source_count == 1 { "" } else { "s" }
        ));
        x.push_str(&format!(
            "<item><title>{}</title><link>{}</link><guid isPermaLink=\"true\">{}</guid><category>{}</category><pubDate>{}</pubDate><description>{}</description></item>\n",
            xml_escape(&s.title),
            xml_escape(&s.url),
            xml_escape(&s.url),
            xml_escape(&s.section_name),
            s.created_at.to_rfc2822(),
            xml_escape(&desc)
        ));
    }
    x.push_str("</channel></rss>\n");
    x
}
