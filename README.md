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

## Savage headlines

Every headline can be rewritten by an AI into a sharper, sarcastic version ("Central bank does nothing, promises to do something next year"). It's built so it never costs speed or trust:

- **Never slows the news.** A story goes out instantly with its original headline; the savage version follows a second or two later and swaps in live.
- **The original is always kept.** Story pages show it under a "Savage take" label, and readers can turn savage headlines off in Settings.
- **Tragedies are left alone.** Stories about deaths, disasters, violence, abuse or serious illness keep their original headline: a keyword filter skips them before any AI call, and the AI is told to refuse anything similar the filter misses.
- **No invented facts.** The AI may only rephrase what the publisher's headline and summary say, and may not mock victims or anyone's identity.
- **Fits free tiers.** Headlines are rewritten in batches of 12, newest first, only for stories under 12 hours old, with a daily cap (3,000 by default).

It's off until the server has an AI key. A **free** key is enough, and no card is needed for either of these:

- **Groq** (recommended): sign in at [console.groq.com](https://console.groq.com/keys), create a key (starts with `gsk_`).
- **Google AI Studio**: sign in at [aistudio.google.com](https://aistudio.google.com/apikey), create a key (starts with `AIza`).

Set it as `LEAK_LLM_API_KEY` on the server; that's all. Leak recognises the provider from the key and uses that provider's free models, best first (Groq: `openai/gpt-oss-120b`, then `openai/gpt-oss-20b`, then `qwen/qwen3.8-27b`). Free tiers have per-minute and per-day limits: when one model hits its limit, Leak rests it and moves to the next, and when all are resting, new stories simply keep their original headlines until the quota resets. Retired models are skipped automatically too.

Paid keys also work (Anthropic `sk-ant-…`, OpenAI, OpenRouter, or any OpenAI-compatible API via `LEAK_LLM_PROVIDER=openai` and `LEAK_LLM_BASE_URL`), and `LEAK_LLM_MODEL` takes a comma-separated list to override the models.

The tone lives in one prompt at the top of `server/src/spice.rs` if you want it meaner or kinder.

## Images

Every story gets a picture through this chain, stopping at the first that works:

1. The image the publisher attaches to the feed item (`media:content`, `media:thumbnail`, enclosure).
2. The first image inside the feed item's text.
3. The article page's own share image (`og:image`), fetched in the background and pushed live when found.
4. For GDELT stories, the share image GDELT reports.
5. If there's still nothing, or the image fails to load, a designed cover in the section's colour with the newsroom's name.

Images load straight from the publisher's servers (`http` links are upgraded to `https` so they work in the apps). Leak deliberately doesn't use AI-generated pictures: a realistic fake photo next to a real news story is misleading.

## Repository layout

| Path | What it is |
|---|---|
| `server/` | The Rust news engine and API (`axum`, `tokio`, `feed-rs`). Also serves the website. `spice.rs` writes savage headlines. |
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

## Run everything for ₹0

Every piece runs on a free tier with **no credit card**:

| Piece | Where | Cost |
|---|---|---|
| News engine (always on) | Render free plan, `render.yaml` | Free: 750 hours/month covers one service all month |
| Website | GitHub Pages | Free for public repos |
| App builds, Docker image | GitHub Actions + GitHub Container Registry | Free for public repos |
| Savage headlines | Groq or Google AI Studio free tier | Free (daily limits) |
| Android app | APK from Releases | Free |
| Windows app | `.exe` from Releases | Free |
| iPhone / iPad | Website added to the Home Screen | Free |

### 1. Put the engine online (Render, ~5 minutes)

1. Sign up at [render.com](https://render.com) with your GitHub account. The free plan needs no card.
2. **New → Blueprint**, choose this repository. Render reads `render.yaml` and creates the `leak-news` web service on the free plan, running the image GitHub Actions already built.
3. When it asks for `LEAK_LLM_API_KEY`, paste your free Groq or Google key (or leave it empty for normal headlines).
4. Open the service's URL, e.g. `https://leak-news.onrender.com`. News appears within a minute.
5. Optional, for automatic deploys: in the service's **Settings → Deploy Hook**, copy the URL and save it as a repository secret named `RENDER_DEPLOY_HOOK`. Then every push to `main` updates the server.

Notes on the free plan: Render sleeps a free service after 15 minutes without visitors, which would stop it catching news, so the engine visits its own address every 10 minutes to stay awake (automatic on Render; `LEAK_KEEPALIVE=0` turns it off). Render can also restart free services at any time; Leak rebuilds its news from the feeds within about a minute. Free bandwidth is limited, so serve the website from GitHub Pages (below) and let Render handle only the API.

### 2. Point the website and apps at it

In **Settings → Secrets and variables → Actions → Variables**, add `LEAK_API_URL` = your Render URL. Then:

- **Website:** **Settings → Pages → Source: GitHub Actions**. The site goes live at `https://mohammedanasasif.github.io/Leak/`.
- **Apps:** the next app build points at your server. Push a tag such as `v0.1.1` to publish the APK and `.exe` on the repository's **Releases** page.

### Other hosts

The image `ghcr.io/mohammedanasasif/leak:latest` runs anywhere Docker does: `docker run -p 8080:8080 ghcr.io/mohammedanasasif/leak`. Set `LEAK_PUBLIC_URL` to the host's https URL to switch on WebSub push (Render's URL is picked up automatically).

## Build the apps with GitHub Actions

Workflow **Build apps** runs on every push to `main` that touches `app/` or `web/`, on any `v*` tag (which also publishes a GitHub Release with the files attached), and manually from the Actions tab. Downloads are on the **Releases** page (or a run's **Artifacts**):

- **Windows:** `Leak-portable.exe` (no install), plus a `…-setup.exe` and `.msi` installer. They aren't code-signed (a certificate costs money), so Windows SmartScreen may say "Windows protected your PC": click **More info → Run anyway**.
- **Android:** `Leak-android.apk`. Allow "install unknown apps" for your browser or file manager when asked. Without signing secrets each build is signed with a throwaway key, so an update needs an uninstall first. To avoid that, make your own free key (`keytool -genkeypair -keystore release.jks -alias leak -keyalg RSA -validity 10000`) and add:

  | Secret | Value |
  |---|---|
  | `ANDROID_KEYSTORE_BASE64` | `base64 -w0 release.jks` |
  | `ANDROID_KEYSTORE_PASSWORD` | keystore password |
  | `ANDROID_KEY_ALIAS` | key alias |
  | `ANDROID_KEY_PASSWORD` | key password (if different) |

- **iPhone / iPad (free):** open the website in Safari → **Share → Add to Home Screen**. Leak then opens full-screen like an app, with live updates. A real App Store or `.ipa` app is only possible with Apple's paid Developer Program ($99/year), so the iOS build job only checks that the code compiles. If you ever join, add `APPLE_DEVELOPMENT_TEAM`, `IOS_CERTIFICATE`, `IOS_CERTIFICATE_PASSWORD` and `IOS_MOBILE_PROVISION` secrets and the job produces `Leak-ios.ipa`.

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
| `LEAK_PUBLIC_URL` | `RENDER_EXTERNAL_URL` on Render | Public https URL; enables WebSub push |
| `LEAK_KEEPALIVE` | on when on Render | Self-ping every 10 min so free hosts don't sleep |
| `LEAK_SOURCES` | `sources.toml` | Source list |
| `LEAK_WEB_DIR` | `../web` | Website folder to serve |
| `LEAK_DATA_DIR` | `data` | Where the restart snapshot is kept |
| `LEAK_DISABLE_GDELT` | – | Set to turn off GDELT |
| `LEAK_LLM_API_KEY` | – | AI key (free Groq/Google keys work); turns on savage headlines |
| `LEAK_LLM_PROVIDER` | from the key | `groq`, `gemini`, `anthropic` or `openai` (any OpenAI-compatible API) |
| `LEAK_LLM_MODEL` | provider's free models | Comma-separated models, best first |
| `LEAK_LLM_BASE_URL` | provider default | API base URL for OpenAI-compatible providers |
| `LEAK_SAVAGE_DAILY_LIMIT` | `3000` | Max headlines rewritten per day |
| `LEAK_SAVAGE` | on | Set to `off` to disable even with a key |

Polling intervals, story limits and image enrichment are under `[settings]` in `sources.toml`.

## Content and rights

Leak shows only the headline, summary and image a publisher puts in its public feed, and every story links to the full article on the publisher's site. It does not copy articles. Feed URLs change over time; `/api/sources` shows any that fail so they can be updated in `sources.toml`.

## Authors

- **Mohammed Anas Asif** ([@MohammedAnasAsif](https://github.com/MohammedAnasAsif)), creator
- Built with Claude (Anthropic)

## License

MIT
