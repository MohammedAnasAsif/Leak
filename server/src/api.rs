use crate::ingest::Ingest;
use crate::model::{Article, LiveEvent, CATEGORIES};
use crate::store::{Query, Store};
use axum::{
    body::Bytes,
    extract::{Path, Query as Q, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Json, Response,
    },
    routing::get,
    Router,
};
use chrono::{DateTime, TimeZone, Utc};
use futures::stream::{self, Stream, StreamExt};
use serde::Deserialize;
use serde_json::json;
use std::{collections::HashMap, convert::Infallible, sync::Arc, time::Duration};
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
};

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Store>,
    pub ingest: Arc<Ingest>,
}

pub fn router(state: AppState, web_dir: &str) -> Router {
    let index = format!("{}/index.html", web_dir.trim_end_matches('/'));
    let api = Router::new()
        .route("/api/health", get(health))
        .route("/api/categories", get(categories))
        .route("/api/articles", get(list))
        .route("/api/articles/{id}", get(one))
        .route("/api/sources", get(sources))
        .route("/websub/{id}", get(websub_verify).post(websub_deliver))
        .with_state(state.clone())
        .layer(CompressionLayer::new());
    // The live stream must not be compressed (buffering would delay events).
    let live = Router::new().route("/api/live", get(live)).with_state(state);
    Router::new()
        .merge(api)
        .merge(live)
        .fallback_service(ServeDir::new(web_dir).fallback(ServeFile::new(index)))
        .layer(CorsLayer::permissive())
}

fn parse_time(s: &str) -> Option<DateTime<Utc>> {
    if let Ok(ms) = s.parse::<i64>() {
        return Utc.timestamp_millis_opt(ms).single();
    }
    DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc))
}

async fn health(State(s): State<AppState>) -> Json<serde_json::Value> {
    let sources = s.store.sources.read();
    let healthy = sources.values().filter(|x| x.last_error.is_none() && x.last_ok.is_some()).count();
    let pushed = sources.values().filter(|x| x.mode == "push").count();
    Json(json!({
        "ok": true,
        "stories": s.store.total(),
        "sources": sources.len(),
        "sources_healthy": healthy,
        "sources_push": pushed,
        "median_detection_secs": s.store.median_lag(),
        "uptime_secs": (Utc::now() - s.store.started).num_seconds(),
        "server_time": Utc::now(),
    }))
}

async fn categories(State(s): State<AppState>) -> Json<serde_json::Value> {
    let counts = s.store.counts();
    let list: Vec<_> = CATEGORIES
        .iter()
        .map(|(slug, name)| json!({"slug": slug, "name": name, "count": counts.get(*slug).copied().unwrap_or(0)}))
        .collect();
    Json(json!(list))
}

#[derive(Deserialize)]
struct ListParams {
    category: Option<String>,
    q: Option<String>,
    before: Option<String>,
    since: Option<String>,
    source: Option<String>,
    limit: Option<usize>,
}

async fn list(State(s): State<AppState>, Q(p): Q<ListParams>) -> Response {
    let category = p.category.as_deref().filter(|c| !c.is_empty() && *c != "all");
    let search = p.q.as_deref().map(str::trim).filter(|q| q.len() >= 2);
    let items = s.store.query(&Query {
        category,
        search,
        before: p.before.as_deref().and_then(parse_time),
        since: p.since.as_deref().and_then(parse_time),
        source: p.source.as_deref(),
        limit: p.limit.unwrap_or(30).clamp(1, 200),
    });
    let mut h = HeaderMap::new();
    h.insert("cache-control", "no-store".parse().unwrap());
    (h, Json(json!({ "articles": items, "server_time": Utc::now() }))).into_response()
}

async fn one(State(s): State<AppState>, Path(id): Path<String>) -> Response {
    let Some(a) = s.store.get(&id) else {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response();
    };
    let related: Vec<Arc<Article>> = s
        .store
        .query(&Query { category: Some(&a.category), search: None, before: None, since: None, source: None, limit: 12 })
        .into_iter()
        .filter(|x| x.id != a.id)
        .take(6)
        .collect();
    Json(json!({ "article": a, "related": related })).into_response()
}

async fn sources(State(s): State<AppState>) -> Json<serde_json::Value> {
    let mut v: Vec<_> = s.store.sources.read().values().cloned().collect();
    v.sort_by(|a, b| a.category.cmp(&b.category).then(a.name.cmp(&b.name)));
    Json(json!(v))
}

/// Server-Sent Events: every new story is pushed to every connected client
/// the moment it is stored. Filtered server-side by `?category=`.
async fn live(
    State(s): State<AppState>,
    Q(p): Q<HashMap<String, String>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let filter = p.get("category").cloned().filter(|c| !c.is_empty() && c != "all");
    let hello = Event::default().event("hello").data(
        json!({
            "server_time": Utc::now(),
            "median_detection_secs": s.store.median_lag(),
            "stories": s.store.total(),
        })
        .to_string(),
    );
    let updates = BroadcastStream::new(s.store.live.subscribe()).filter_map(move |msg| {
        let filter = filter.clone();
        async move {
            let ev = match msg {
                Ok(ev) => ev,
                Err(BroadcastStreamRecvError::Lagged(n)) => {
                    // Client fell behind; tell it to refetch instead of silently dropping.
                    return Some(Ok(Event::default().event("resync").data(n.to_string())));
                }
            };
            let (name, a) = match ev {
                LiveEvent::New(a) => ("story", a),
                LiveEvent::Update(a) => ("update", a),
            };
            if filter.as_deref().map_or(false, |c| c != a.category) {
                return None;
            }
            let data = serde_json::to_string(&*a).ok()?;
            Some(Ok(Event::default().event(name).id(a.id.clone()).data(data)))
        }
    });
    let stream = stream::once(async move { Ok(hello) }).chain(updates);
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)).text("ping"))
}

#[derive(Deserialize)]
struct VerifyParams {
    #[serde(rename = "hub.mode")]
    mode: String,
    #[serde(rename = "hub.topic")]
    topic: String,
    #[serde(rename = "hub.challenge")]
    challenge: Option<String>,
    #[serde(rename = "hub.lease_seconds")]
    lease: Option<i64>,
}

async fn websub_verify(State(s): State<AppState>, Path(id): Path<String>, Q(p): Q<VerifyParams>) -> Response {
    match s.ingest.websub_verify(&id, &p.mode, &p.topic, p.challenge.as_deref().unwrap_or(""), p.lease) {
        Some(ch) => (StatusCode::OK, ch).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn websub_deliver(
    State(s): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let sig = headers.get("x-hub-signature-256").or_else(|| headers.get("x-hub-signature")).and_then(|v| v.to_str().ok());
    let sig = sig.map(|s| s.to_string());
    let ingest = s.ingest.clone();
    // Ack immediately; parse off the request path.
    tokio::task::spawn_blocking(move || ingest.websub_deliver(&id, sig.as_deref(), &body));
    StatusCode::ACCEPTED
}
