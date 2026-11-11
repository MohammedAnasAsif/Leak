# Roadmap

Ordered by value. Everything stays free to run.

## Speed

- **Social-signal breaking detection.** Listen to Bluesky's public firehose (Jetstream, free, no account) in live mode and flag sudden spikes in mentions of a place or name, often minutes before outlets publish.
- **WebSub push.** Subscribe to feeds that support WebSub so they push new items instead of waiting for the next poll.
- **GDELT.** Add the free GDELT 2.0 API as a source for wider coverage in 65 languages.
- **Adaptive polling** in live mode: check fast-moving feeds every 30 seconds and slow ones every 10 minutes, using the measured delay already shown on the status page.

## Quality

- **Better grouping with embeddings.** Swap TF-IDF for a small multilingual sentence-embedding model run in Rust (candle or ort) to group paraphrases and cross-language coverage of the same event.
- **Story summaries in Leak's own words.** Generate a neutral two-sentence summary from all outlets' headlines with a small open model running locally in live mode (for example via Ollama), clearly labelled as automated.
- **Merge stories** that split early and later turn out to be the same event (keep the old link redirecting).
- **Outlet context.** Country, ownership and funding notes per outlet, contributed and sourced by the community.

## Product

- **Topic pages and follow alerts.** Permanent pages per ongoing topic; follow a topic to get notified (ntfy or web push).
- **Web push notifications** for breaking stories (free with VAPID keys and a small push sender in live mode).
- **Regional and language editions** (Hindi, Spanish, Arabic, Portuguese, French first).
- **Home-screen widgets** on Android and iOS showing the top three headlines.
- **World map view** of where today's stories are happening.
- **Embeddable headline widget** for other sites.
- **Weekly email digest** using a free newsletter tier.
- **Audio brief as a podcast feed**, generated with an open text-to-speech model.
- **Archive** beyond seven days, stored as compressed JSON in a separate branch.

## Apps

- **Android back button and share target** (share any link into Leak to see who else is covering it).
- **Native notifications** in the Windows and Android apps.
- **AltStore source** for iPhone sideloading with automatic updates.
