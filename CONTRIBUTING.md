# Contributing to Leak

Thanks for helping. The quickest ways in:

- **Add a news source** from a country or region Leak covers thinly. See [docs/SOURCES.md](docs/SOURCES.md).
- **Improve section keywords** in `config/site.toml` when you spot a story in the wrong section.
- **Pick a roadmap item** from [docs/ROADMAP.md](docs/ROADMAP.md).

## Development

```bash
cargo test --release          # unit tests, including clustering on sample stories
cargo run --release -- demo   # build the demo site into ./public (no network needed)
cargo run --release -- serve  # live mode on http://localhost:8080
```

Before opening a pull request, run `cargo fmt` and `cargo clippy --release -- -D warnings`. CI runs both.

## Where things live

- `engine/src/fetch.rs` fetching and parsing feeds
- `engine/src/cluster.rs` deciding which articles are the same story
- `engine/src/views.rs` ranking, sections, breaking detection, tags
- `engine/src/render.rs` writing pages, JSON, RSS and sitemap
- `web/templates/` page templates (MiniJinja)
- `web/static/css/site.css` and `web/static/js/app.js` the whole front end, no build step

## Changing clustering

`engine/src/cluster.rs` has tests with real-world style headline pairs that must and must not group, and `engine/src/demo.rs` checks that every sample event becomes exactly one story. Add a test case for any grouping mistake you fix.

## Design rules

Two typefaces (Newsreader for reading, Schibsted Grotesk for interface), the wire-yellow highlighter is the only loud element, sentence case everywhere, no images from outlets, and every page must work on a 360-pixel-wide phone with keyboard focus visible.
