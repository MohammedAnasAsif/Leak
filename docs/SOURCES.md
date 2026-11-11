# News sources

All sources live in [`config/sources.toml`](../config/sources.toml). Each is a public RSS or Atom feed.

## Adding a source

1. Find the outlet's RSS/Atom feed (often linked as "RSS" in the site footer, or at `/feed`, `/rss`, `/rss.xml`).
2. Add an entry:
   ```toml
   [[source]]
   name = "Outlet Name"
   url = "https://example.com/feed.xml"
   section = "world"        # world, politics, finance, tech, science, health, climate, sports, culture
   country = "KE"           # ISO country code of where the outlet is based
   region = "Africa"        # Africa, Americas, Asia, Europe, Middle East, Oceania, Global
   weight = 1.0             # 0.5–1.5; higher means its headline is preferred for a story
   ```
3. Check it: `cargo run --release -- sources`.
4. Open a pull request with "source" in the title.

Use `section = "world"` for general feeds: their stories are routed to the right section by the keywords in `config/site.toml`. Specialist feeds (a tech site, a sports desk) should use their own section.

## What makes a good source

- Original reporting, not another aggregator.
- Regions and countries Leak covers thinly today: Africa, Latin America, Central and Southeast Asia, the Middle East.
- An English feed for now; other languages are on the roadmap.

## What Leak shows from a feed

Headline, a summary trimmed to about 300 characters, the outlet name, time, and a link to the original article. Never full articles, never images. This is the standard way feed readers and aggregators use RSS.

## Removal requests

Publishers can ask for their feed to be removed by opening an issue titled "Remove <outlet name>". Removal requests are honoured promptly and without argument.

## Feed health

The site's About page lists every source with its status and typical delay, refreshed on every run. Failing feeds are skipped automatically and never break a build.
