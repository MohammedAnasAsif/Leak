# Leak.

**The world, the moment it happens.** A real-time world news magazine for the web, Windows, Android and iOS, powered by a news engine written in Rust.

Leak watches **92 publisher feeds** across 9 sections (World, Politics, Finance, Tech, Science, Health, Sports, Entertainment, Climate) plus **GDELT**'s global event stream, and pushes every new story to every open app within seconds. Stories are presented as blog-style posts in an editorial layout inspired by the Newslora magazine template.

```
 Publishers ──WebSub push (instant)──┐
 92 RSS/Atom feeds ──poll 20–60 s────┤                 ┌─▶ Website (served by the engine or GitHub Pages)
 GDELT world events ──every 2 min────┼─▶  Leak engine ─┼─▶ Windows app (.exe)
                                     │   (Rust, axum)  ├─▶ Android app (.apk)
                                     │   dedupe, rank  └─▶ iOS app
                                     └── live stream (Server-Sent Events) to every client
```

## How "fast" works

No app can know about an event before someone reports it, so Leak aims to be the fastest at *noticing* reports:

- **Push where publishers support it.** Feeds that advertise a WebSub hub are subscribed to, and the hub calls Leak the instant the publisher posts. Enable it by setting `LEAK_PUBLIC_URL` to the server's https address.
- **Aggressive, polite polling everywhere else.** Wires and breaking desks every 20 s, everything else every 60 s, using conditional requests (ETag / Last-Modified) so unchanged feeds cost almost nothing, with backoff for failing feeds.
- **Instant fan-out.** New stories are broadcast over Server-Sent Events, so the website and apps update without refreshing. A breaking ticker, "new stories" pill and optional system notifications surface them.
- **Measured, not claimed.** Every story records when it was published and when Leak detected it. The median detection delay is shown in the site footer and at `/api/health`, and each article shows "Picked up N s after publication".

## Repository layout

| Path | What it is |
|---|---|
| `server/` | The Rust news engine and API (`axum`, `tokio`, `feed-rs`). Also serves the website. |
| `server/sources.toml` | Every feed and GDELT query. Add, remove or re-categorise freely. |
| `web/` | The UI: plain HTML/CSS/JS, no build step. Shared by the website and all apps. |
| `app/src-tauri/` | Tauri v2 shell that turns `web/` into Windows, macOS, Linux, Android and iOS apps. |
| `.github/workflows/` | CI, app builds, Docker image + deploy, optional GitHub Pages. |

## Run it locally

```bash
cd server
cargo run --release          # http://localhost:8080
```

Open http://localhost:8080. Stories appear within a minute as feeds are read. Feed health is at http://localhost:8080/api/sources.

Desktop app against your local server:

```bash
cd app && npm install
npx tauri dev                # then Settings → News server → http://localhost:8080
```

## Deploy the server (needed by the apps)

The apps need a public **https** address for the engine.

1. **Image:** every push to `main` builds `ghcr.io/mohammedanasasif/leak:latest` (workflow *Server image + deploy*). Run it anywhere: `docker run -p 8080:8080 -v leak-data:/data ghcr.io/mohammedanasasif/leak`.
2. **Fly.io (optional, automated):** install `flyctl`, run `fly launch --no-deploy --copy-config` once (choose a unique app name and update `fly.toml`), then add a `FLY_API_TOKEN` repository secret (`fly tokens create deploy`). Each push to `main` then deploys.
3. Set `LEAK_PUBLIC_URL` (in `fly.toml` `[env]` or your host) to that https URL to switch on WebSub push.

## Build the apps with GitHub Actions

Workflow **Build apps** runs on every push to `main` that touches `app/` or `web/`, on any `v*` tag (which also creates a GitHub Release with the files attached), and manually from the Actions tab.

First, in **Settings → Secrets and variables → Actions → Variables**, add:

| Variable | Value |
|---|---|
| `LEAK_API_URL` | Your server's https URL, e.g. `https://leak-news.fly.dev` |

Then download from the run's **Artifacts**:

- **Windows:** `Leak-portable.exe` (no install), plus an NSIS `…-setup.exe` and `.msi` installer.
- **Android:** `Leak-android.apk`. Without signing secrets it's signed with a throwaway CI key, which installs fine but means later builds need an uninstall first. For stable updates, add your own keystore:

  | Secret | Value |
  |---|---|
  | `ANDROID_KEYSTORE_BASE64` | `base64 -w0 release.jks` |
  | `ANDROID_KEYSTORE_PASSWORD` | keystore password |
  | `ANDROID_KEY_ALIAS` | key alias |
  | `ANDROID_KEY_PASSWORD` | key password (if different) |

- **iOS:** Apple only lets signed apps onto iPhones, which needs an Apple Developer account ($99/yr). Without it, the iOS job just confirms the app compiles for iOS. With it, add these secrets and the job produces `Leak-ios.ipa`:

  | Secret | Value |
  |---|---|
  | `APPLE_DEVELOPMENT_TEAM` | Your 10-character Team ID |
  | `IOS_CERTIFICATE` | base64 of your distribution `.p12` |
  | `IOS_CERTIFICATE_PASSWORD` | its password |
  | `IOS_MOBILE_PROVISION` | base64 of the provisioning profile |

  The export method defaults to `release-testing` (ad-hoc, install on registered devices); set the variable `IOS_EXPORT_METHOD` to `app-store-connect` for TestFlight/App Store.

**Website:** the engine serves it at its own URL. To also publish a free static copy on GitHub Pages, set `LEAK_API_URL` and choose **Settings → Pages → Source: GitHub Actions**.

## API

| Endpoint | Returns |
|---|---|
| `GET /api/articles?category=tech&q=&before=&since=&limit=30` | Newest stories (filter, search, paginate with `before`) |
| `GET /api/articles/{id}` | One story plus related stories |
| `GET /api/categories` | Sections with story counts |
| `GET /api/live?category=` | Server-Sent Events: `story`, `update`, `resync` |
| `GET /api/sources` | Health of every source (push/poll, last error) |
| `GET /api/health` | Totals and median detection delay |

## Configuration

| Env var | Default | Meaning |
|---|---|---|
| `PORT` | `8080` | Listen port |
| `LEAK_PUBLIC_URL` | – | Public https URL; enables WebSub push |
| `LEAK_SOURCES` | `sources.toml` | Source list |
| `LEAK_WEB_DIR` | `../web` | Website folder to serve |
| `LEAK_DATA_DIR` | `data` | Where the restart snapshot is kept |
| `LEAK_DISABLE_GDELT` | – | Set to turn off GDELT |

Polling intervals, story limits and image enrichment are under `[settings]` in `sources.toml`.

## Content and rights

Leak shows only the headline, summary and image a publisher puts in its public feed, and every story links to the full article on the publisher's site. It does not copy articles. Feed URLs change over time; `/api/sources` shows any that fail so they can be updated in `sources.toml`.

## License

MIT
