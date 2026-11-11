# Leak

**World news, the moment it breaks.** An open-source news wire that watches outlets in every region of the world, groups their reports about the same event into one story, and shows you every outlet's take side by side. Website, Android app, Windows app and iPhone app, all from one Rust codebase. Costs nothing to run.

- **Fast.** The engine is written in Rust: fetching 56 feeds, grouping thousands of articles and writing the whole site takes about a second. The free hosted setup publishes every 5 minutes; live mode on any computer updates every minute and pushes new stories to open pages instantly.
- **Every angle.** Each story page lists who is reporting it, from which region, and how it spread, with links to the original reporting.
- **Private.** No accounts, ads, analytics or cookies. Fonts are self-hosted. Saved stories never leave your device.
- **Free forever.** GitHub Actions + GitHub Pages host everything. No servers, no API keys, no paid services.

## What you get

| Platform | How it's delivered |
|---|---|
| Website | GitHub Pages, rebuilt every 5 minutes, installable as an app (PWA), works offline |
| Android | `Leak.apk` on the Releases page, built by GitHub Actions |
| Windows | `Leak-setup.exe` and `Leak-portable.exe` on the Releases page |
| iPhone | Sideload with [xtool](https://xtool.sh) (free Apple ID), or Add to Home Screen from Safari — see [docs/IOS.md](docs/IOS.md) |

Built-in features: top stories ranked by global coverage, nine sections (World, Politics, Finance, Tech, Science, Health, Climate, Sports, Culture), a live wire where fresh headlines glow and fade, breaking and developing flags, story pages with a coverage map by region and a spread timeline, a daily ten-story brief you can listen to, search, saved stories, "new since your last visit" markers, dark mode, keyboard shortcuts (`/`, `j`, `k`), RSS feeds per section, a JSON API, and auto-posting to Telegram, Bluesky, Mastodon and ntfy.

## Get it running (about 10 minutes, all free)

1. **Push this code** to your GitHub repository (public, so Actions minutes are unlimited).
2. **Turn on Pages:** repository Settings → Pages → Build and deployment → Source: **GitHub Actions**.
3. **Start the news:** Actions tab → *Publish news* → **Run workflow**. After that it runs by itself every 5 minutes. Your site appears at `https://<you>.github.io/<repo>/`.
4. **Build the apps:** Actions tab → *Release apps* → **Run workflow**. In about 15 minutes the Releases page has the `.apk` and `.exe`. For updatable Android installs, add a signing key first ([docs/RELEASING.md](docs/RELEASING.md)).
5. **Optional:** add bot secrets to auto-post big stories ([docs/GROWTH.md](docs/GROWTH.md#2-free-distribution-bots-already-built)).

## Run it on your computer

Needs [Rust](https://rustup.rs).

```bash
cargo run --release -- demo            # offline demo with invented sample news → ./public
cargo run --release -- run             # fetch real news once → ./public
cargo run --release -- serve           # live mode: refresh every 60 s, serve on http://localhost:8080
cargo run --release -- sources         # check every feed in config/sources.toml
```

Open `public/index.html` after `demo` or `run`. Live mode is the fastest setup: it fetches every minute and pushes "new stories" to open pages the moment they land. Expose it for free with a [Cloudflare Tunnel](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/do-more-with-tunnels/trycloudflare/) if you want the public to use it.

## How it works

```
 56 RSS/Atom feeds ──▶ fetch (concurrent, conditional GET, 12 s timeout)
                         │
                         ▼
                      ingest: dedupe by canonical URL, drop stale items
                         │
                         ▼
                      cluster: TF-IDF headline vectors, name-aware matching,
                         │      incremental so story links never change
                         ▼
                      views: section routing, ranking (outlets × regions × recency),
                         │    breaking/developing flags, tags, related stories
                         ▼
                      render: HTML pages, JSON API, RSS, sitemap, PWA files
                         │
                         ├──▶ GitHub Pages (every 5 min)  or  live server (every 60 s + push)
                         └──▶ Telegram / Bluesky / Mastodon / ntfy (big stories only)
```

The apps are thin native shells around the live site (Tauri 2 for Windows and Android, SwiftUI for iOS), so a website update reaches every app instantly with no reinstall.

## Project layout

```
config/          site.toml (name, sections, keywords) and sources.toml (feeds)
engine/          the Rust engine: fetch, cluster, rank, render, serve, distribute
web/             templates/, static/ (CSS, JS, fonts, icons) and the service worker
apps/shell/      Tauri 2 app for Windows (.exe) and Android (.apk)
apps/ios/        SwiftUI app for iPhone, built and sideloaded with xtool
.github/         publish (every 5 min), CI, and app release workflows
docs/            growth playbook, roadmap, iOS, releasing, sources
scripts/         configure.sh to point a fork's apps at its own site
```

## About speed, honestly

News reaches Leak as fast as outlets publish it to their feeds, typically within a few minutes. The free hosted setup adds up to 5 minutes (GitHub's shortest schedule, sometimes delayed under load). Live mode cuts that to about a minute. The status page shows each outlet's measured delay. [docs/ROADMAP.md](docs/ROADMAP.md) covers the next speed steps (social-signal detection, WebSub push).

## Publishers and content

Leak shows headlines and short summaries from public RSS feeds, credits the outlet beside each one, and links to the original article. It never republishes full articles or images. Any publisher can ask for removal by opening an issue; see [docs/SOURCES.md](docs/SOURCES.md).

## Contributing

Add a source, fix a bug, translate, or pick something from the roadmap. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. Fonts (Newsreader, Schibsted Grotesk) are under the SIL Open Font License.
