use anyhow::Result;
use clap::{Parser, Subcommand};
use leak::config::Config;
use leak::render::RenderOptions;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "leak", version, about = "Open-source world news wire")]
struct Cli {
    /// Directory with site.toml and sources.toml
    #[arg(long, global = true, default_value = "config")]
    config: PathBuf,
    /// Directory with templates/, static/ and sw.js
    #[arg(long, global = true, default_value = "web")]
    web: PathBuf,
    /// Persistent state file (articles, stories, feed health)
    #[arg(long, global = true, default_value = "state/state.json")]
    state: PathBuf,
    /// Output directory for the generated site
    #[arg(long, global = true, default_value = "public")]
    out: PathBuf,
    /// Public URL of the site, e.g. https://you.github.io/leak
    #[arg(long, global = true, env = "LEAK_BASE_URL")]
    base_url: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Fetch every feed once, update stories, and write the site
    Run {
        /// Post big stories to Telegram/Bluesky/Mastodon/ntfy (needs env vars)
        #[arg(long)]
        distribute: bool,
    },
    /// Re-render the site from saved state without fetching
    Build,
    /// Live mode: fetch on an interval, serve the site, push updates to readers
    Serve {
        #[arg(long, default_value_t = 8080)]
        port: u16,
        /// Seconds between fetch cycles
        #[arg(long, default_value_t = 60)]
        interval: u64,
        #[arg(long)]
        distribute: bool,
    },
    /// Build a demo site from built-in sample data (no network)
    Demo,
    /// Fetch every feed and print a health table (for maintaining sources.toml)
    Sources,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "leak=info".into()),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();
    let mut cfg = Config::load(&cli.config)?;
    if let Some(b) = &cli.base_url {
        cfg.site.base_url = b.trim_end_matches('/').to_string();
    }
    let opts = |live: Option<String>, demo: bool| RenderOptions {
        web_dir: cli.web.clone(),
        out_dir: cli.out.clone(),
        live_endpoint: live,
        demo,
    };

    match cli.cmd {
        Cmd::Run { distribute } => {
            leak::cycle(&cfg, &cli.state, &opts(None, false), distribute).await?;
        }
        Cmd::Build => {
            let state = leak::model::State::load_or_default(&cli.state)?;
            let views = leak::views::build(&state, &cfg, chrono::Utc::now());
            let r = leak::render::render(&cfg, &state, &views, &opts(None, false))?;
            tracing::info!("{} stories, {} files written", r.stories, r.files_written);
        }
        Cmd::Serve {
            port,
            interval,
            distribute,
        } => {
            if cli.base_url.is_none() {
                cfg.site.base_url = format!("http://localhost:{port}");
            }
            leak::serve::serve(
                cfg,
                opts(Some("api/live".into()), false),
                leak::serve::ServeOptions {
                    port,
                    interval_secs: interval,
                    state_path: cli.state.clone(),
                    distribute,
                },
            )
            .await?;
        }
        Cmd::Demo => {
            let r = leak::demo::build(&cfg, &opts(None, true))?;
            tracing::info!("demo site: {} stories in {}", r.stories, cli.out.display());
        }
        Cmd::Sources => {
            let results = leak::fetch::fetch_all(&cfg, &Default::default()).await;
            let mut ok = 0;
            for r in &results {
                let src = cfg.source(&r.source).expect("known source");
                match &r.health.last_error {
                    None => {
                        ok += 1;
                        println!("ok    {:>4} items  {}", r.items.len(), src.name);
                    }
                    Some(e) => println!("FAIL             {}  ({})  {}", src.name, src.url, e),
                }
            }
            println!("\n{ok}/{} feeds healthy", results.len());
        }
    }
    Ok(())
}
