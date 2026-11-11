//! Leak: an open-source world news wire.
//!
//! Pipeline: fetch feeds → ingest (dedupe + cluster into stories) → views (rank,
//! sections, breaking) → render (static HTML, JSON API, RSS) → distribute (bots).

pub mod cluster;
pub mod config;
pub mod cover;
pub mod demo;
pub mod distribute;
pub mod fetch;
pub mod ingest;
pub mod model;
pub mod render;
pub mod serve;
pub mod text;
pub mod views;

use anyhow::Result;
use std::path::Path;

/// One full fetch → publish cycle, persisting state. Used by `run` and `serve`.
pub async fn cycle(
    cfg: &config::Config,
    state_path: &Path,
    render_opts: &render::RenderOptions,
    distribute: bool,
) -> Result<ingest::IngestStats> {
    let started = std::time::Instant::now();
    let mut state = model::State::load_or_default(state_path)?;
    let results = fetch::fetch_all(cfg, &state.feeds).await;
    let fetched_in = started.elapsed();
    let now = chrono::Utc::now();
    let stats = ingest::ingest(&mut state, cfg, results, now);
    let views = views::build(&state, cfg, now);
    let report = render::render(cfg, &state, &views, render_opts)?;
    if distribute {
        if let Err(e) = distribute::run(cfg, &mut state, &views, now).await {
            tracing::warn!("distribution failed: {e:#}");
        }
    }
    state.save(state_path)?;
    tracing::info!(
        "feeds {}/{} ok, {} items, +{} articles, +{} stories, {} grown, {} live stories, {} files written; fetch {:.1}s, total {:.1}s",
        stats.feeds_ok,
        stats.feeds_ok + stats.feeds_failed,
        stats.items_seen,
        stats.articles_added,
        stats.stories_created,
        stats.stories_grown,
        report.stories,
        report.files_written,
        fetched_in.as_secs_f32(),
        started.elapsed().as_secs_f32()
    );
    Ok(stats)
}
