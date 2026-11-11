//! Live mode: one process that fetches every minute, rebuilds the site, serves it,
//! and pushes "new stories" events to open pages over server-sent events.

use crate::config::Config;
use crate::model::State;
use crate::render::{render, RenderOptions};
use anyhow::Result;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::{extract::State as AxState, routing::get, Router};
use futures::stream::Stream;
use std::convert::Infallible;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};

pub struct ServeOptions {
    pub port: u16,
    pub interval_secs: u64,
    pub state_path: PathBuf,
    pub distribute: bool,
}

pub async fn serve(cfg: Config, render_opts: RenderOptions, opts: ServeOptions) -> Result<()> {
    let (tx, _) = broadcast::channel::<String>(32);
    let out_dir = render_opts.out_dir.clone();

    // Build once from saved state so the server has pages immediately.
    {
        let state = State::load_or_default(&opts.state_path)?;
        let views = crate::views::build(&state, &cfg, chrono::Utc::now());
        render(&cfg, &state, &views, &render_opts)?;
    }

    let loop_tx = tx.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(opts.interval_secs.max(15)));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            match crate::cycle(&cfg, &opts.state_path, &render_opts, opts.distribute).await {
                Ok(stats) if stats.articles_added > 0 => {
                    let msg = serde_json::json!({
                        "generated": chrono::Utc::now().to_rfc3339(),
                        "new_stories": stats.stories_created,
                        "new_articles": stats.articles_added,
                    });
                    let _ = loop_tx.send(msg.to_string());
                }
                Ok(_) => {}
                Err(e) => tracing::error!("cycle failed: {e:#}"),
            }
        }
    });

    let app = Router::new()
        .route("/api/live", get(live))
        .with_state(tx)
        .fallback_service(
            ServeDir::new(&out_dir).not_found_service(ServeFile::new(out_dir.join("404.html"))),
        )
        .layer(CompressionLayer::new());

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], opts.port));
    tracing::info!("serving on http://localhost:{}", opts.port);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

async fn live(
    AxState(tx): AxState<broadcast::Sender<String>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = BroadcastStream::new(tx.subscribe())
        .filter_map(|m| m.ok())
        .map(|m| Ok(Event::default().event("update").data(m)));
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(20)))
}
