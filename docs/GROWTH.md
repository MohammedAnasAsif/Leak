# Growing Leak: the zero-budget playbook

Every idea here costs nothing. Ideas marked **Built** already work in this codebase; the rest are ordered roughly by impact per hour of effort.

## 1. Reasons people come back (built into the product)

These are the habits the product is designed around. Each one gives people a reason to open Leak again tomorrow, without dark patterns.

- **Built: "New to you" markers.** Stories that appeared since your last visit are flagged, so returning always rewards you.
- **Built: The live wire.** Headlines glow when fresh and fade over 45 minutes. Opening Leak always shows what changed in the last hour at a glance.
- **Built: New-stories pill.** Pages check for news every minute and offer "Show 3 new stories" instead of reloading under you.
- **Built: Today's brief.** Ten stories in about a minute, with a Listen button (on-device text-to-speech, free and private). Ideal for a commute habit.
- **Built: Every angle on one page.** The story page shows how outlets in different regions report the same event. Few news apps do this, and it's the main reason to choose Leak over a single outlet.
- **Built: How it spread.** A timeline of which outlet reported first and how coverage spread. Satisfying for news junkies and very shareable.
- **Built: Breaking and developing flags.** Detected automatically when several outlets pile onto a story within minutes.
- **Built: Saved stories, search, dark mode, keyboard shortcuts, offline reading.**
- **Built: Installable everywhere.** PWA on every phone, plus Android APK, Windows EXE and iOS sideload.

## 2. Free distribution bots (already built)

The engine posts the biggest stories (3+ outlets, under 3 hours old, max 3 per run) wherever you add credentials as repository secrets (Settings → Secrets and variables → Actions). Every post links back to the Leak story page, which is the growth loop.

| Channel | Secrets to add | Setup |
|---|---|---|
| **Telegram channel** | `TELEGRAM_BOT_TOKEN`, `TELEGRAM_CHAT_ID` | Create a bot with @BotFather, make it an admin of a public channel, use `@yourchannel` as the chat id. Telegram news channels grow fast, especially in India, the Middle East and Eastern Europe. |
| **Bluesky** | `BLUESKY_HANDLE`, `BLUESKY_APP_PASSWORD` | Create an account, then Settings → App passwords. Posts include a link card. Bluesky's audience actively wants news accounts. |
| **Mastodon** | `MASTODON_INSTANCE`, `MASTODON_TOKEN` | Use an instance that allows bots (e.g. mastodon.social with the bot flag on). Preferences → Development → New application with `write:statuses`. |
| **Push notifications (ntfy)** | `NTFY_TOPIC` | Pick an unguessable topic name. Readers install the free ntfy app and subscribe. Note that anyone who knows a public topic name can also post to it. |

Start with Telegram and Bluesky; they bring the most readers for the least effort.

## 3. Launch week (one post per day, all free)

1. **Show HN** on Hacker News: "Show HN: Leak – open-source world news wire in Rust, rebuilt every 5 minutes for $0". Post on a weekday morning US time. Answer every comment.
2. **Reddit**, one community per day, each post tailored: r/rust (the engine and clustering), r/opensource, r/selfhosted (live mode), r/androidapps, r/InternetIsBeautiful (the site), r/dataisbeautiful (a chart of how a big story spread). Read each subreddit's rules first.
3. **Product Hunt** launch on a Tuesday–Thursday, with screenshots of the story page and the wire.
4. **Lobsters, dev.to, Hashnode**: a write-up of "how I cluster news in Rust with TF-IDF for free" brings developers and backlinks.
5. **Bluesky and X**: a short screen recording of the wire lighting up during a breaking story. Motion sells this product.
6. **LinkedIn**: the build-in-public story (open source, $0 infrastructure, Rust).
7. **Awesome lists**: open pull requests to awesome-rust, awesome-selfhosted (news), awesome-tauri, awesome-pwa.

## 4. App store presence (free)

- **F-Droid**: the free, open-source Android store. Leak qualifies (MIT, no trackers). Submit a merge request to fdroiddata. This alone brings steady, privacy-minded installs.
- **Obtainium**: lets people install and auto-update straight from GitHub Releases. Add an "Add to Obtainium" badge in the README.
- **IzzyOnDroid repo**: faster than F-Droid to get listed.
- **Windows**: submit to winget (free, a pull request to winget-pkgs) so people can `winget install Leak`. For signed builds, apply to the free SignPath Foundation programme for open-source projects.
- **AltStore / SideStore** source for iPhone users, generated from the same release.

## 5. Search and discovery (mostly built)

- **Built:** a page per story with a clean title, description and canonical link; sitemap.xml updated every run; robots.txt; RSS for every section; social preview cards.
- **Submit the sitemap** to Google Search Console and Bing Webmaster Tools (both free). Bing also feeds DuckDuckGo and many AI search tools.
- **Topic pages** (roadmap): permanent pages like /topic/port-amara collect every story on an ongoing subject and rank for it over weeks.
- **Embeddable widget** (roadmap): a "latest headlines" box any blogger can paste in. Each embed is a backlink and a stream of readers.

## 6. Community

- **Make sources a contribution magnet.** Adding a feed is a five-line pull request. Label issues "good first issue: add a source for <country>". Contributors become promoters.
- **Regional editions by volunteers.** Someone who adds 15 Kenyan or Brazilian outlets owns that edition and shares it locally.
- **Translations.** The interface has very little text, so translating is quick. A Hindi, Spanish, Arabic or Portuguese edition each opens a new audience.
- **Public roadmap** (GitHub Projects) and a GitHub Discussions board so users can vote on features.
- **Credit contributors** on the About page.

## 7. Partnerships that cost nothing

- **Student journalism and media-literacy groups**: the "every angle" view is a teaching tool. Offer it to teachers.
- **Open-source newsletters** (This Week in Rust, Console.dev, Changelog) feature interesting projects for free.
- **Other aggregators and bots**: offer the JSON API and RSS to Discord bot makers and Telegram channel owners. Every integration credits Leak.

## 8. Ideas for later (see ROADMAP.md)

Topic follow alerts, a weekly email digest, "explain this story" summaries from a local open model, multi-language editions, home-screen widgets, bias and ownership info per outlet, a world map of where stories are breaking, and an audio daily brief published as a podcast feed.

## What to measure, without tracking anyone

Leak has no analytics by design. Use signals that don't track individuals: GitHub stars and release download counts, Telegram/Bluesky follower counts, Search Console impressions, and F-Droid install stats.
