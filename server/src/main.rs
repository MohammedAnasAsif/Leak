mod api;
mod config;
mod ingest;
mod model;
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
