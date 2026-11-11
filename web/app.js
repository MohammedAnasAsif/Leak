// Leak — shared client for the website and the Tauri apps (Windows, Android, iOS).
// No framework, no build step: this exact folder is what the server and the apps ship.

const CFG = window.LEAK_CONFIG || {};
const TAURI = window.__TAURI__ || null;
const store = {
  get(k, d) { try { const v = localStorage.getItem(k); return v == null ? d : JSON.parse(v); } catch { return d; } },
  set(k, v) { try { localStorage.setItem(k, JSON.stringify(v)); } catch {} },
};

let API = (store.get("leak.server", "") || CFG.apiBase || "").replace(/\/$/, "");
const NAMES = {
  world: "World", politics: "Politics", finance: "Finance", tech: "Tech", science: "Science",
  health: "Health", sports: "Sports", entertainment: "Entertainment", climate: "Climate",
};
const CATS = Object.keys(NAMES);

const $ = (s, el = document) => el.querySelector(s);
const $$ = (s, el = document) => [...el.querySelectorAll(s)];
const main = $("#main");

const state = {
  latest: [],          // newest-first stories for ticker / just-in
  pending: 0,          // live stories that arrived while reading
  lastDetected: null,  // for catch-up after reconnect
  route: null,
  loadingMore: false,
};

/* ───────────────────────── utilities ───────────────────────── */
const esc = (s = "") => String(s).replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));

function ago(iso) {
  const t = new Date(iso).getTime();
  const s = Math.max(0, (Date.now() - t) / 1000);
  if (s < 45) return "just now";
  if (s < 3600) return `${Math.round(s / 60)}m ago`;
  if (s < 86400) return `${Math.round(s / 3600)}h ago`;
  return new Date(t).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}
const isFresh = iso => Date.now() - new Date(iso).getTime() < 30 * 60 * 1000;
const fmtTime = iso => new Date(iso).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
const fmtFull = iso => new Date(iso).toLocaleString(undefined, { weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

async function api(path) {
  const r = await fetch(API + path, { headers: { accept: "application/json" } });
  if (!r.ok) throw new Error(`HTTP ${r.status}`);
  return r.json();
}

function openExternal(url) {
  if (TAURI?.opener?.openUrl) return TAURI.opener.openUrl(url);
  window.open(url, "_blank", "noopener,noreferrer");
}

/* ───────────────────────── rendering ───────────────────────── */
function media(a, { live = false, eager = false } = {}) {
  const ph = `<div class="placeholder"><b>${esc(a.source)}</b><small>${esc(NAMES[a.category] || "")}</small></div>`;
  const img = a.image
    ? `<img src="${esc(a.image)}" alt="" ${eager ? "" : 'loading="lazy"'} decoding="async" referrerpolicy="no-referrer" class="loading" onload="this.classList.remove('loading')" onerror="this.replaceWith(Object.assign(document.createElement('div'),{className:'placeholder',innerHTML:this.dataset.ph}))" data-ph="${esc(ph)}">`
    : ph;
  return `<div class="media">${img}${live && isFresh(a.published) ? '<span class="badge-live">Live</span>' : ""}</div>`;
}

function meta(a, { src = true } = {}) {
  const fresh = isFresh(a.published);
  return `<div class="meta">
    ${src ? `<span class="src">${esc(a.source)}</span><span class="sep"></span>` : ""}
    <time data-ts="${a.published}" class="${fresh ? "fresh" : ""}">${ago(a.published)}</time>
    <span class="sep"></span><span>${a.read_minutes} min read</span>
  </div>`;
}

const href = a => `#/a/${a.id}`;

function card(a, variant = "std") {
  if (variant === "row") {
    return `<a class="card row" href="${href(a)}" data-id="${a.id}">${media(a)}<div>
      <span class="tag">${esc(NAMES[a.category])}</span><h3>${esc(a.title)}</h3>${meta(a)}</div></a>`;
  }
  const dek = a.summary && variant !== "compact" ? `<p>${esc(a.summary)}</p>` : "";
  return `<a class="card ${variant}" href="${href(a)}" data-id="${a.id}">${media(a, { live: true })}
    <div><span class="tag">${esc(NAMES[a.category])}</span><h3>${esc(a.title)}</h3></div>${dek}${meta(a)}</a>`;
}

function justInItem(a, cls = "") {
  return `<li class="${cls}" data-id="${a.id}"><a href="${href(a)}">${esc(a.title)}</a>
    <div class="meta"><span class="src">${esc(a.source)}</span><span class="sep"></span><time data-ts="${a.published}" class="${isFresh(a.published) ? "fresh" : ""}">${ago(a.published)}</time></div></li>`;
}

function skeleton() {
  main.innerHTML = `<div class="hero">
    <div><div class="skeleton" style="aspect-ratio:4/3"></div><div class="skeleton" style="height:48px;margin-top:16px"></div><div class="skeleton" style="height:48px;margin-top:10px;width:70%"></div></div>
    <div class="stack"><div class="skeleton" style="aspect-ratio:16/10"></div><div class="skeleton" style="aspect-ratio:16/10"></div></div>
    <div class="justin">${"<div class='skeleton' style='height:52px;margin-bottom:14px'></div>".repeat(7)}</div></div>`;
}

function errorView(e) {
  main.innerHTML = `<div class="empty"><h2>Can't reach the newsroom</h2>
    <p>${esc(e.message || e)}. ${TAURI ? "Check your connection or the server address in Settings." : "Check your connection and try again."}</p>
    <p><button class="btn" onclick="location.reload()">Retry</button></p></div>`;
}

/* Prefer real stories with a picture and a summary for the big slots. */
function pickFeatured(list, n, used = new Set()) {
  const score = a => (a.image ? 2 : 0) + (a.summary ? 1 : 0) + (a.via === "gdelt" ? -1 : 0);
  const pool = list.slice(0, 40).filter(a => !used.has(a.id)).sort((x, y) => score(y) - score(x) || new Date(y.published) - new Date(x.published));
  const out = pool.slice(0, n);
  out.forEach(a => used.add(a.id));
  return out;
}

async function renderHome() {
  skeleton();
  const [{ articles: latest }, ...perCat] = await Promise.all([
    api("/api/articles?limit=80"),
    ...CATS.map(c => api(`/api/articles?category=${c}&limit=6`).then(r => [c, r.articles]).catch(() => [c, []])),
  ]);
  state.latest = latest;
  updateTicker();
  if (!latest.length) {
    main.innerHTML = `<div class="empty"><h2>Warming up the wires</h2><p>The engine is fetching its first stories. This page fills in by itself.</p></div>`;
    return;
  }
  const used = new Set();
  const [lead] = pickFeatured(latest, 1, used);
  const stack = pickFeatured(latest, 2, used);
  const justIn = latest.slice(0, 12);

  let html = `<section class="hero">
    <a class="lead card" href="${href(lead)}" data-id="${lead.id}">${media(lead, { live: true, eager: true })}
      <div><span class="tag">${esc(NAMES[lead.category])}</span><h1 class="h-serif">${esc(lead.title)}</h1>
      ${lead.summary ? `<p class="dek">${esc(lead.summary)}</p>` : ""}${meta(lead)}</div></a>
    <div class="stack">${stack.map(a => card(a, "compact")).join("")}</div>
    <aside class="justin" aria-label="Just in"><h2 class="col-title"><i class="dot"></i>Just in</h2>
      <ol id="justIn">${justIn.map(a => justInItem(a)).join("")}</ol></aside>
  </section>`;

  for (const [cat, list] of perCat) {
    const items = list.filter(a => !used.has(a.id)).slice(0, 4);
    if (!items.length) continue;
    const [big, ...side] = items;
    html += `<section class="section" data-cat="${cat}">
      <div class="section-head"><h2>${NAMES[cat]}</h2><a class="more" href="#/c/${cat}">View all <span aria-hidden="true">→</span></a></div>
      <div class="section-grid"><div class="big">${card(big)}</div>
      <div class="side">${side.map(a => card(a, "row")).join("")}</div></div></section>`;
  }
  main.innerHTML = html;
}

async function renderCategory(cat) {
  main.innerHTML = `<header class="page-head"><span class="tag">Section</span><h1 class="h-serif">${NAMES[cat]}</h1><p id="catCount"></p></header><div class="grid" id="grid"></div><div class="loadmore"><button class="btn ghost" id="moreBtn">Load more</button></div>`;
  const { articles } = await api(`/api/articles?category=${cat}&limit=24`);
  $("#grid").innerHTML = articles.length ? articles.map(a => card(a)).join("") : `<div class="empty"><p>No ${NAMES[cat]} stories yet — they'll appear here live.</p></div>`;
  $("#catCount").textContent = `Live from ${new Set(articles.map(a => a.source)).size} newsrooms · updated ${articles[0] ? ago(articles[0].published) : "—"}`;
  wireLoadMore(`/api/articles?category=${cat}&limit=24`, articles);
}

async function renderSearch(q) {
  main.innerHTML = `<header class="page-head"><span class="tag">Search</span><h1 class="h-serif">“${esc(q)}”</h1><p id="catCount"></p></header><div class="grid" id="grid"></div><div class="loadmore"><button class="btn ghost" id="moreBtn">Load more</button></div>`;
  const base = `/api/articles?q=${encodeURIComponent(q)}&limit=24`;
  const { articles } = await api(base);
  $("#catCount").textContent = articles.length ? `${articles.length}${articles.length === 24 ? "+" : ""} stories` : "";
  $("#grid").innerHTML = articles.length ? articles.map(a => card(a)).join("") : `<div class="empty"><h2>Nothing yet</h2><p>No stories match. New ones arrive every few seconds.</p></div>`;
  wireLoadMore(base, articles);
}

function wireLoadMore(base, first) {
  const btn = $("#moreBtn");
  let last = first[first.length - 1];
  if (!last || first.length < 24) { btn.parentElement.remove(); return; }
  const load = async () => {
    if (state.loadingMore || !last) return;
    state.loadingMore = true; btn.textContent = "Loading…";
    try {
      const { articles } = await api(`${base}&before=${encodeURIComponent(last.published)}`);
      $("#grid").insertAdjacentHTML("beforeend", articles.map(a => card(a)).join(""));
      last = articles[articles.length - 1];
      if (articles.length < 24) btn.parentElement.remove(); else btn.textContent = "Load more";
    } finally { state.loadingMore = false; }
  };
  btn.onclick = load;
  new IntersectionObserver(es => es.some(e => e.isIntersecting) && load(), { rootMargin: "600px" }).observe(btn);
}

async function renderArticle(id) {
  main.innerHTML = `<div class="article"><div class="skeleton" style="height:60px;margin:30px 0"></div><div class="skeleton" style="aspect-ratio:16/9"></div></div>`;
  let data;
  try { data = await api(`/api/articles/${id}`); }
  catch { main.innerHTML = `<div class="empty"><h2>This story has rolled off the wire</h2><p><a class="btn" href="#/">Back to the front page</a></p></div>`; return; }
  const { article: a, related } = data;
  document.title = `${a.title} — Leak`;
  const lag = Math.max(0, Math.round((new Date(a.detected) - new Date(a.published)) / 1000));
  const via = { push: "Instant push (WebSub)", poll: "Live feed", gdelt: "GDELT global monitor" }[a.via] || a.via;
  const initial = (a.source || "?").trim()[0];
  const body = a.summary
    ? `<p>${esc(a.summary)}</p>`
    : `<p>${esc(a.source)} is reporting this story. Leak picked it up from the global news stream ${lag < 120 ? "moments" : ago(a.published).replace(" ago", "")} after it was published; the full report is on ${esc(a.source)}'s site.</p>`;
  main.innerHTML = `<article class="article">
    <nav class="crumbs"><a href="#/">Home</a><span>/</span><a href="#/c/${a.category}">${NAMES[a.category]}</a></nav>
    <span class="tag">${NAMES[a.category]}</span>
    <h1 class="h-serif">${esc(a.title)}</h1>
    <div class="byline">
      <div class="who"><span class="avatar">${esc(initial)}</span><div>
        <div><strong>${esc(a.author || a.source)}</strong>${a.author ? ` <span class="muted">· ${esc(a.source)}</span>` : ""}</div>
        <div class="meta"><time data-ts="${a.published}">${ago(a.published)}</time><span class="sep"></span><span>${a.read_minutes} min read</span></div>
      </div></div>
      <button class="btn ghost" id="shareBtn">Share</button>
    </div>
    ${a.image ? media(a, { eager: true }) : ""}
    <div class="body">${body}</div>
    <aside class="facts"><h4>The brief</h4><dl>
      <dt>Source</dt><dd>${esc(a.source)} <span class="muted">(${esc(a.source_domain)})</span></dd>
      <dt>Published</dt><dd>${fmtFull(a.published)}</dd>
      <dt>Picked up</dt><dd>${lag <= 5 ? "Instantly" : lag < 3600 ? `${lag < 60 ? lag + " s" : Math.round(lag / 60) + " min"} after publication` : fmtFull(a.detected)}</dd>
      <dt>Via</dt><dd>${via}</dd>
    </dl></aside>
    <div class="cta"><a class="btn" href="${esc(a.url)}" data-external>Read the full story on ${esc(a.source)} <span aria-hidden="true">↗</span></a></div>
    <p class="note">Leak shows the headline and summary the publisher shares in its public feed. The full article, and all rights to it, belong to ${esc(a.source)}.</p>
    ${related.length ? `<section class="related"><h2>More in ${NAMES[a.category]}</h2><div class="grid">${related.map(r => card(r)).join("")}</div></section>` : ""}
  </article>`;
  $("#shareBtn").onclick = async () => {
    const payload = { title: a.title, text: a.title, url: a.url };
    try {
      if (navigator.share) await navigator.share(payload);
      else { await navigator.clipboard.writeText(a.url); $("#shareBtn").textContent = "Link copied"; }
    } catch {}
  };
}

/* ───────────────────────── routing ───────────────────────── */
function parseRoute() {
  const h = location.hash.replace(/^#/, "") || "/";
  const [path, qs] = h.split("?");
  const parts = path.split("/").filter(Boolean);
  if (parts[0] === "c" && CATS.includes(parts[1])) return { name: "cat", cat: parts[1] };
  if (parts[0] === "a" && parts[1]) return { name: "article", id: parts[1] };
  if (parts[0] === "search") return { name: "search", q: new URLSearchParams(qs).get("q") || "" };
  return { name: "home" };
}

async function route() {
  const r = parseRoute();
  state.route = r;
  state.pending = 0; updatePill();
  $$("#nav a").forEach(a => a.classList.toggle("active", a.dataset.cat === (r.cat || (r.name === "home" ? "all" : ""))));
  if (r.name !== "article") document.title = r.name === "cat" ? `${NAMES[r.cat]} — Leak` : "Leak — The world, the moment it happens";
  window.scrollTo({ top: 0 });
  try {
    if (r.name === "cat") await renderCategory(r.cat);
    else if (r.name === "article") await renderArticle(r.id);
    else if (r.name === "search") await renderSearch(r.q);
    else await renderHome();
  } catch (e) { errorView(e); }
  main.focus({ preventScroll: true });
}

/* ───────────────────────── live updates ───────────────────────── */
function updateTicker() {
  const recent = state.latest.filter(a => Date.now() - new Date(a.published) < 90 * 60 * 1000).slice(0, 14);
  const t = $("#ticker");
  if (recent.length < 3) { t.hidden = true; return; }
  const items = recent.map(a => `<a href="${href(a)}"><time>${fmtTime(a.published)}</time>${esc(a.title)}</a>`).join("");
  $("#tickerRun").innerHTML = items + items; // duplicated for a seamless loop
  t.hidden = false;
}

function updatePill() {
  const p = $("#newPill");
  if (state.pending > 0) { p.textContent = `↑ ${state.pending} new ${state.pending === 1 ? "story" : "stories"}`; p.hidden = false; }
  else p.hidden = true;
}

function onStory(a) {
  state.latest.unshift(a);
  state.latest.length = Math.min(state.latest.length, 120);
  state.lastDetected = a.detected;
  updateTicker();
  notify(a);
  const r = state.route || {};
  if (r.name === "home") {
    const list = $("#justIn");
    if (list) {
      list.insertAdjacentHTML("afterbegin", justInItem(a, "enter"));
      while (list.children.length > 12) list.lastElementChild.remove();
    }
    if (window.scrollY > 500) { state.pending++; updatePill(); }
  } else if (r.name === "cat" && r.cat === a.category) {
    const grid = $("#grid");
    if (grid && window.scrollY < 300) grid.insertAdjacentHTML("afterbegin", card(a));
    else { state.pending++; updatePill(); }
  }
}

function onUpdate(a) {
  // A picture arrived for a story already on screen.
  $$(`[data-id="${a.id}"] .media`).forEach(m => {
    if (!m.querySelector("img") && a.image) {
      const fresh = document.createElement("div");
      fresh.innerHTML = media(a);
      m.replaceWith(fresh.firstElementChild);
    }
  });
}

let es, wasConnected = false;
function connectLive() {
  try { es?.close(); } catch {}
  es = new EventSource(API + "/api/live");
  const status = $("#liveStatus");
  es.addEventListener("hello", async ev => {
    status.classList.add("on");
    const h = JSON.parse(ev.data);
    setStats(h);
    if (wasConnected && state.lastDetected) {
      // Catch up on anything we missed while disconnected.
      try {
        const { articles } = await api(`/api/articles?since=${encodeURIComponent(state.lastDetected)}&limit=100`);
        articles.reverse().forEach(onStory);
      } catch {}
    }
    wasConnected = true;
  });
  es.addEventListener("story", ev => onStory(JSON.parse(ev.data)));
  es.addEventListener("update", ev => onUpdate(JSON.parse(ev.data)));
  es.addEventListener("resync", () => route());
  es.onerror = () => { status.classList.remove("on"); $("#liveText").textContent = "Reconnecting…"; };
}

function setStats(h) {
  if (h.sources != null) {
    $("#liveText").textContent = `Live · ${h.sources_healthy ?? h.sources} sources`;
    $("#statSources").textContent = h.sources_healthy ?? h.sources;
  } else {
    $("#liveText").textContent = "Live";
  }
  if (h.stories != null) $("#statStories").textContent = h.stories.toLocaleString();
  if (h.median_detection_secs != null) {
    const s = h.median_detection_secs;
    $("#statLag").textContent = s < 90 ? `${s}s` : `${Math.round(s / 60)} min`;
  }
}

async function refreshHealth() {
  try { setStats(await api("/api/health")); } catch {}
}

/* ───────────────────────── notifications ───────────────────────── */
const alerts = () => store.get("leak.alerts", { on: false, cats: ["world", "politics", "finance"] });

async function ensurePermission() {
  if (TAURI?.notification) {
    const n = TAURI.notification;
    if (await n.isPermissionGranted()) return true;
    return (await n.requestPermission()) === "granted";
  }
  if (!("Notification" in window)) return false;
  if (Notification.permission === "granted") return true;
  return (await Notification.requestPermission()) === "granted";
}

let lastNotify = 0;
function notify(a) {
  const cfg = alerts();
  if (!cfg.on || !cfg.cats.includes(a.category)) return;
  if (Date.now() - new Date(a.published) > 15 * 60 * 1000) return;
  if (Date.now() - lastNotify < 20000) return; // at most one alert every 20 s
  lastNotify = Date.now();
  const title = `${NAMES[a.category]} · ${a.source}`;
  if (TAURI?.notification) { TAURI.notification.sendNotification({ title, body: a.title }); return; }
  if ("Notification" in window && Notification.permission === "granted" && document.hidden) {
    const n = new Notification(title, { body: a.title, icon: "icon.svg", tag: a.id });
    n.onclick = () => { window.focus(); location.hash = href(a); };
  }
}

/* ───────────────────────── chrome ───────────────────────── */
function buildChrome() {
  $("#nav").innerHTML = `<a href="#/" data-cat="all">Top stories</a>` + CATS.map(c => `<a href="#/c/${c}" data-cat="${c}">${NAMES[c]}</a>`).join("");
  $("#footerNav").innerHTML = CATS.map(c => `<li><a href="#/c/${c}">${NAMES[c]}</a></li>`).join("");
  $("#today").textContent = new Date().toLocaleDateString(undefined, { weekday: "long", day: "numeric", month: "long", year: "numeric" });
  $("#year").textContent = new Date().getFullYear();

  const theme = store.get("leak.theme", null);
  if (theme) document.documentElement.dataset.theme = theme;
  $("#themeBtn").onclick = () => {
    const dark = document.documentElement.dataset.theme
      ? document.documentElement.dataset.theme === "dark"
      : matchMedia("(prefers-color-scheme: dark)").matches;
    const next = dark ? "light" : "dark";
    document.documentElement.dataset.theme = next;
    store.set("leak.theme", next);
  };

  const sd = $("#searchDialog");
  $("#searchBtn").onclick = () => { sd.showModal(); $("#searchInput").focus(); };
  $("#searchForm").onsubmit = () => { const q = $("#searchInput").value.trim(); if (q) location.hash = `#/search?q=${encodeURIComponent(q)}`; };
  sd.addEventListener("click", e => { if (e.target === sd) sd.close(); });
  document.addEventListener("keydown", e => { if (e.key === "/" && !/input|textarea/i.test(document.activeElement.tagName)) { e.preventDefault(); $("#searchBtn").click(); } });

  const st = $("#settingsDialog");
  $("#serverField").hidden = !TAURI;
  $("#alertCats").innerHTML += CATS.map(c => `<label><input type="checkbox" value="${c}">${NAMES[c]}</label>`).join("");
  $("#settingsBtn").onclick = () => {
    const cfg = alerts();
    $("#alertsToggle").checked = cfg.on;
    $$("#alertCats input").forEach(i => (i.checked = cfg.cats.includes(i.value)));
    $("#serverInput").value = store.get("leak.server", "");
    st.showModal();
  };
  st.addEventListener("click", e => { if (e.target === st) st.close(); });
  $("#settingsSave").onclick = async () => {
    const on = $("#alertsToggle").checked && (await ensurePermission());
    store.set("leak.alerts", { on, cats: $$("#alertCats input:checked").map(i => i.value) });
    const server = $("#serverInput").value.trim().replace(/\/$/, "");
    if (TAURI && server !== store.get("leak.server", "")) {
      store.set("leak.server", server);
      API = (server || CFG.apiBase || "").replace(/\/$/, "");
      connectLive(); route();
    }
  };

  $("#newPill").onclick = () => route();

  // Publisher links open in the system browser (in the apps) or a new tab (on the web).
  document.addEventListener("click", e => {
    const a = e.target.closest("a[data-external]");
    if (a) { e.preventDefault(); openExternal(a.href); }
  });

  setInterval(() => $$("time[data-ts]").forEach(t => {
    t.textContent = ago(t.dataset.ts);
    t.classList.toggle("fresh", isFresh(t.dataset.ts));
  }), 30000);
  setInterval(refreshHealth, 60000);
  document.addEventListener("visibilitychange", () => { if (!document.hidden && es?.readyState === 2) connectLive(); });
}

buildChrome();
window.addEventListener("hashchange", route);
route();
connectLive();
refreshHealth();
