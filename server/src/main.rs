mod api;
mod config;
mod ingest;
mod model;
mod spice;
mod store;
mod util;

use config::{Env, SourcesFile};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "leak=info,tower_http=warn".into()),
        )
        .init();

    let env = Env::from_env();
    let sources = SourcesFile::load(std::path::Path::new(&env.sources_path))?;
    tracing::info!(
        feeds = sources.feeds.len(),
        gdelt_queries = sources.gdelt.len(),
        push = env.public_url.is_some(),
        "starting Leak news engine"
    );

    let store = store::Store::new(sources.settings.max_articles, &env.data_dir);
    let restored = store.load_snapshot();
    if restored > 0 {
        tracing::info!(restored, "restored stories from snapshot");
    }

    let ingest = ingest::Ingest::new(store.clone(), sources.settings.clone(), env.clone(), &sources.feeds);
    ingest.spawn_pollers();
    ingest.spawn_gdelt(sources.gdelt.clone());
    ingest.spawn_enricher();

    match env.llm.clone() {
        Some(cfg) => spice::Spice::new(cfg, store.clone()).spawn(),
        None => tracing::info!("savage headlines off (set LEAK_LLM_API_KEY to enable)"),
    }

    // Free hosts (Render) put a service to sleep after ~15 idle minutes, which
    // would stop the engine from catching news. A light self-ping through the
    // public URL every 10 minutes keeps it awake. On by default on Render;
    // LEAK_KEEPALIVE=1/0 forces it on/off elsewhere.
    let keepalive = match std::env::var("LEAK_KEEPALIVE").ok().as_deref() {
        Some("0" | "off" | "false") => false,
        Some(_) => true,
        None => std::env::var("RENDER").is_ok(),
    };
    if let (true, Some(public)) = (keepalive, env.public_url.clone()) {
        tracing::info!(url = %public, "keep-alive ping every 10 min");
        tokio::spawn(async move {
            let http = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("http client");
            let url = format!("{public}/api/health?keepalive=1");
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(600)).await;
                if let Err(e) = http.get(&url).send().await {
                    tracing::warn!("keep-alive ping failed: {e}");
                }
            }
        });
    }

    // Periodic snapshot so restarts are instant.
    {
        let store = store.clone();
        tokio::spawn(async move {
            let mut t = tokio::time::interval(Duration::from_secs(60));
            t.tick().await;
            loop {
                t.tick().await;
                let s = store.clone();
                let _ = tokio::task::spawn_blocking(move || s.save_snapshot()).await;
            }
        });
    }

    let app = api::router(api::AppState { store: store.clone(), ingest }, &env.web_dir);
    let listener = tokio::net::TcpListener::bind(&env.bind).await?;
    tracing::info!("listening on http://{}", env.bind);
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("shutting down, saving snapshot");
            store.save_snapshot();
        })
        .await?;
    Ok(())
}
