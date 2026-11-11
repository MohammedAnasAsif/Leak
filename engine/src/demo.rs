//! Offline demo data. Every outlet, place and company here is fictional, so the
//! demo can never be mistaken for real reporting.

use crate::config::{Config, Source};
use crate::fetch::{FetchResult, RawItem};
use crate::model::{FeedHealth, State};
use crate::render::{render, RenderOptions, Report};
use anyhow::Result;
use chrono::{Duration, Utc};

const SOURCES: &[(&str, &str, &str, &str, f64)] = &[
    // name, section, country, region, weight
    ("The Meridian Times", "world", "XA", "Americas", 1.2),
    ("Harbour Daily", "world", "XE", "Europe", 1.1),
    ("Kestrel Wire", "world", "XG", "Global", 1.0),
    ("Sahel Observer", "world", "XF", "Africa", 1.0),
    ("Pacific Ledger", "world", "XO", "Oceania", 0.9),
    ("Indus Chronicle", "world", "XI", "Asia", 1.0),
    ("Gulf Standard", "world", "XM", "Middle East", 1.0),
    ("Andes Herald", "world", "XC", "Americas", 0.9),
    ("Lumen Tech", "tech", "XA", "Americas", 1.0),
    ("Ledger & Ticker", "finance", "XE", "Europe", 1.0),
    ("Pitchside", "sports", "XE", "Europe", 0.9),
    ("Stagelight", "culture", "XA", "Americas", 0.9),
    ("Field Notes", "science", "XG", "Global", 1.0),
    ("Pulse Health", "health", "XG", "Global", 1.0),
    ("Thaw", "climate", "XE", "Europe", 1.0),
    ("Capitol Brief", "politics", "XA", "Americas", 1.0),
];

/// (source index, minutes ago, headline, snippet)
type Item = (usize, i64, &'static str, &'static str);

const EVENTS: &[&[Item]] = &[
    &[
        (2, 24, "Magnitude 7.1 earthquake strikes off the coast of Port Amara", "A strong undersea earthquake struck off Port Amara early on Wednesday, according to the regional seismic agency."),
        (4, 19, "Port Amara hit by powerful 7.1 quake, tsunami warning issued", "Authorities issued a tsunami warning for coastal districts and told residents to move inland."),
        (1, 15, "Tsunami warning after strong earthquake rocks Port Amara coast", "Waves of up to one metre were expected to reach the shore within the hour."),
        (5, 9, "Port Amara earthquake: residents told to move to higher ground", "Shelters have opened in schools and stadiums across the city."),
        (0, 6, "Strong quake off Port Amara triggers tsunami alert across the region", "Neighbouring islands also raised alerts as a precaution."),
        (6, 3, "Port Amara quake: airport closed as tsunami alert stays in force", "Flights were diverted while engineers inspect the runway."),
    ],
    &[
        (0, 290, "Valdoria and Kestran agree ceasefire after talks in Geneva", "The two governments agreed to halt fighting along the border from midnight."),
        (1, 260, "Valdoria, Kestran sign truce following Geneva negotiations", "Mediators said monitors would be deployed within a week."),
        (6, 230, "Ceasefire between Valdoria and Kestran welcomed by regional leaders", "Leaders across the region urged both sides to keep to the deal."),
        (3, 200, "Displaced families cautious as Valdoria-Kestran ceasefire begins", "Aid groups said roads to border towns remain unsafe."),
        (5, 150, "What the Valdoria-Kestran ceasefire means for trade routes", "Shipping firms are waiting before restoring routes through the strait."),
    ],
    &[
        (9, 95, "Norland central bank holds interest rates at 4.25%", "The bank kept its key rate unchanged and signalled cuts could come next year."),
        (1, 80, "Norland keeps rates unchanged at 4.25 per cent as inflation cools", "Inflation fell to 2.6% last month, the lowest in three years."),
        (0, 70, "Norland rate decision: central bank holds steady, hints at 2027 cuts", ""),
    ],
    &[
        (8, 52, "Corvid Systems unveils AI chip it says runs twice as fast", "The company says the chip uses half the power of its previous model."),
        (0, 41, "Corvid Systems' new AI chip takes aim at data-centre rivals", "Shares rose 6% in early trading after the announcement."),
        (5, 33, "Corvid Systems AI chip to be made at new Indus Valley plant", "The plant will employ around 4,000 people when it opens."),
    ],
    &[
        (1, 200, "Tessaly election: Reform Alliance wins largest share of seats", "Turnout reached 71%, the highest in two decades."),
        (0, 190, "Reform Alliance leads in Tessaly election but falls short of majority", "Coalition talks are expected to take weeks."),
        (15, 170, "Tessaly election results: what a hung parliament means", "No party won a majority, so the Reform Alliance must now find coalition partners."),
        (2, 120, "Tessaly coalition talks begin after inconclusive election", "The Reform Alliance has invited two smaller parties to talks."),
    ],
    &[
        (13, 140, "Cholera outbreak reported in Lake Mira region", "Health officials confirmed 120 cases in the past week."),
        (3, 110, "Lake Mira cholera cases rise as clean water runs short", "Aid agencies are sending water purification kits."),
    ],
    &[
        (14, 330, "Southern Valdoria breaks heat record for third day running", "Temperatures reached 46.2C in the city of Orsk."),
        (1, 300, "Heatwave grips Southern Valdoria as record falls again", "Schools closed early and outdoor work was restricted."),
        (6, 260, "Valdoria heatwave: power grid strained as demand hits record", ""),
    ],
    &[
        (10, 75, "Norland win Tri-Nations final by six wickets", "Captain Ida Brenn hit an unbeaten 88 to seal the title."),
        (5, 60, "Brenn's unbeaten 88 seals Tri-Nations title for Norland", ""),
        (10, 50, "Tri-Nations cricket: Brenn named player of the tournament", "The Norland captain scored 412 runs across the series."),
    ],
    &[
        (11, 400, "Quiet Harbour wins top prize at Coralline Film Festival", "The debut feature from director Mara Ostin took the Golden Shell."),
        (1, 380, "Debut director Mara Ostin takes Golden Shell at Coralline festival", ""),
    ],
    &[
        (12, 220, "Telescope finds signs of water vapour on exoplanet Tessaly-b", "The planet orbits a small star 41 light years away."),
        (0, 210, "Water vapour detected on distant exoplanet Tessaly-b, astronomers say", "The team says more observations are needed to confirm the finding."),
        (2, 180, "Tessaly-b water vapour discovery: why it matters", ""),
    ],
    &[
        (9, 160, "Halcyon Bank agrees merger with Northgate in €9bn deal", "The combined bank would be the region's third largest."),
        (0, 150, "Halcyon and Northgate banks to merge, creating regional giant", ""),
    ],
    &[
        (8, 125, "Riverlight streaming service confirms data breach affecting 3m users", "Email addresses and viewing histories were exposed, the company said."),
        (1, 100, "Riverlight data breach: what users should do now", "Users are advised to change passwords and watch for phishing emails."),
    ],
    &[
        (3, 85, "Floods displace thousands in Doru after days of heavy rain", "The river burst its banks overnight, flooding three districts."),
        (14, 70, "Doru floods: rescue teams reach cut-off villages", ""),
        (0, 58, "Floods in Doru force thousands from their homes", "The national weather service expects more rain this weekend."),
    ],
    &[
        (7, 240, "Port workers strike in Cordera, halting copper exports", "The union is demanding higher wages and safer conditions."),
        (9, 210, "Cordera port strike pushes copper prices to six-month high", ""),
        (9, 45, "Commodity prices slip as Cordera supply fears ease", "Copper fell 1.4% in morning trading as talks with the union resumed."),
    ],
    &[(10, 300, "Valdoria FC break transfer record with signing of striker Teo Lund", "The fee is reported to be the largest in the league's history.")],
    &[(11, 520, "National Museum of Tessaly returns 40 artefacts to Port Amara", "The objects were taken during the colonial period.")],
    &[
        (13, 350, "Malaria vaccine rollout reaches one million children in Sahel states", "Health ministers say cases among under-fives have fallen sharply."),
        (3, 330, "One million children vaccinated against malaria across Sahel region", ""),
    ],
    &[
        (15, 180, "Norland parliament passes landmark data privacy law", "Companies will have to delete user data on request within 30 days."),
        (1, 150, "Norland's new privacy law: what changes for tech companies", ""),
    ],
    &[
        (12, 90, "Lantern lander launches on mission to the Moon's south pole", "The lander is due to touch down in 11 days."),
        (2, 80, "Lantern Moon lander lifts off on south pole mission", ""),
    ],
    &[(6, 130, "Gulf states announce joint desalination project", "The plants will supply water to two million people by 2030.")],
    &[(5, 210, "Indus Valley metro opens new line connecting airport and city centre", "The 28-kilometre line cuts journey times to 35 minutes.")],
    &[(4, 260, "Pacific islands seek climate finance at regional summit", "Leaders asked for faster access to adaptation funds.")],
    &[(7, 310, "Andes Herald investigation reveals delays in Cordera rebuilding funds", "")],
    &[(11, 140, "Composer Lio Varga announces farewell tour", "The tour will visit 24 cities over two years.")],
    &[(13, 240, "Study links daily walking to lower heart disease risk", "Researchers followed 40,000 adults for ten years.")],
    &[(14, 420, "Coral reef recovery seen in Tessaly marine reserve", "")],
];

pub fn build(base_cfg: &Config, opts: &RenderOptions) -> Result<Report> {
    let mut cfg = base_cfg.clone();
    cfg.sources = SOURCES
        .iter()
        .map(|(name, section, country, region, weight)| Source {
            name: name.to_string(),
            url: format!("https://demo.invalid/{}", crate::text::slugify(name, 4)),
            section: section.to_string(),
            country: country.to_string(),
            region: region.to_string(),
            weight: *weight,
            id: String::new(),
        })
        .collect();
    cfg.finish()?;

    let now = Utc::now();
    let mut per_source: Vec<Vec<RawItem>> = vec![Vec::new(); cfg.sources.len()];
    for (e, event) in EVENTS.iter().enumerate() {
        for (i, (src, mins, title, snippet)) in event.iter().enumerate() {
            per_source[*src].push(RawItem {
                source: cfg.sources[*src].id.clone(),
                url: format!("https://demo.invalid/story/{e}/{i}"),
                title: title.to_string(),
                snippet: snippet.to_string(),
                published: Some(now - Duration::minutes(*mins)),
            });
        }
    }
    let results = per_source
        .into_iter()
        .enumerate()
        .map(|(i, items)| FetchResult {
            source: cfg.sources[i].id.clone(),
            health: FeedHealth {
                last_ok: Some(now),
                last_items: items.len(),
                lag_minutes: Some(1.0 + (i % 5) as f64),
                ..Default::default()
            },
            items,
        })
        .collect();

    let mut state = State::default();
    crate::ingest::ingest(&mut state, &cfg, results, now);
    let views = crate::views::build(&state, &cfg, now);
    render(&cfg, &state, &views, opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_events_cluster_as_written() {
        let mut cfg = Config::load(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../config"
        )))
        .unwrap();
        cfg.sources = SOURCES
            .iter()
            .map(|(n, s, c, r, w)| Source {
                name: n.to_string(),
                url: format!("https://demo.invalid/{n}"),
                section: s.to_string(),
                country: c.to_string(),
                region: r.to_string(),
                weight: *w,
                id: String::new(),
            })
            .collect();
        cfg.finish().unwrap();
        let now = Utc::now();
        let mut results: Vec<FetchResult> = Vec::new();
        for (e, event) in EVENTS.iter().enumerate() {
            for (i, (src, mins, title, snippet)) in event.iter().enumerate() {
                results.push(FetchResult {
                    source: cfg.sources[*src].id.clone(),
                    items: vec![RawItem {
                        source: cfg.sources[*src].id.clone(),
                        url: format!("https://demo.invalid/{e}/{i}"),
                        title: title.to_string(),
                        snippet: snippet.to_string(),
                        published: Some(now - Duration::minutes(*mins)),
                    }],
                    health: FeedHealth::default(),
                });
            }
        }
        let mut state = State::default();
        crate::ingest::ingest(&mut state, &cfg, results, now);
        // Every event should become exactly one story.
        let mut wrong = Vec::new();
        for (e, event) in EVENTS.iter().enumerate() {
            let ids: std::collections::HashSet<String> = (0..event.len())
                .map(|i| {
                    state.articles
                        [&crate::config::short_hash(&format!("https://demo.invalid/{e}/{i}"), 16)]
                        .story
                        .clone()
                })
                .collect();
            if ids.len() != 1 {
                wrong.push(format!(
                    "event {e} split into {} stories: {}",
                    ids.len(),
                    event[0].2
                ));
            }
        }
        let mut owner: std::collections::HashMap<String, usize> = Default::default();
        for (e, event) in EVENTS.iter().enumerate() {
            for i in 0..event.len() {
                let sid = state.articles
                    [&crate::config::short_hash(&format!("https://demo.invalid/{e}/{i}"), 16)]
                    .story
                    .clone();
                if let Some(prev) = owner.insert(sid, e) {
                    if prev != e {
                        wrong.push(format!(
                            "events {prev} and {e} merged: '{}' + '{}'",
                            EVENTS[prev][0].2, event[i].2
                        ));
                    }
                }
            }
        }
        assert_eq!(
            state.stories.len(),
            EVENTS.len(),
            "story count; problems: {wrong:?}"
        );
        assert!(wrong.is_empty(), "{wrong:#?}");

        let views = crate::views::build(&state, &cfg, now);
        let quake = views
            .stories
            .iter()
            .find(|s| s.title.contains("Port Amara") && s.source_count >= 5)
            .unwrap();
        assert!(quake.breaking, "quake should be breaking");
        assert_eq!(views.stories[0].id, quake.id, "breaking quake should lead");
        let chip = views
            .stories
            .iter()
            .find(|s| s.title.contains("Corvid"))
            .unwrap();
        assert_eq!(chip.section, "tech");
        let rates = views
            .stories
            .iter()
            .find(|s| s.title.contains("Norland") && s.title.to_lowercase().contains("rate"))
            .unwrap();
        assert_eq!(rates.section, "finance");
    }
}
