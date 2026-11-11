/* Leak service worker: instant repeat visits and offline reading.
   Pages and the JSON API are network-first (news must be fresh), with the cached
   copy as a fallback; versioned assets are cache-first. */
const VERSION = "__VERSION__";
const STATIC = "leak-static-" + VERSION;
const PAGES = "leak-pages";
const PRECACHE = ["./", "./index.html", "./offline.html", "./brief.html", "./saved.html", "./search.html"];

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches.open(PAGES).then((c) => c.addAll(PRECACHE)).then(() => self.skipWaiting())
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches.keys()
      .then((keys) => Promise.all(keys.filter((k) => k.startsWith("leak-static-") && k !== STATIC).map((k) => caches.delete(k))))
      .then(() => self.clients.claim())
  );
});

function timeout(ms) {
  return new Promise((_, reject) => setTimeout(() => reject(new Error("timeout")), ms));
}

async function networkFirst(request) {
  const cache = await caches.open(PAGES);
  try {
    const response = await Promise.race([fetch(request), timeout(6000)]);
    if (response.ok) cache.put(request, response.clone());
    trim(cache, 300);
    return response;
  } catch (e) {
    const hit = await cache.match(request, { ignoreSearch: request.url.includes("/api/") });
    if (hit) return hit;
    if (request.mode === "navigate") return (await cache.match("./offline.html")) || Response.error();
    return Response.error();
  }
}

async function cacheFirst(request) {
  const cache = await caches.open(STATIC);
  const hit = await cache.match(request);
  if (hit) return hit;
  const response = await fetch(request);
  if (response.ok) cache.put(request, response.clone());
  return response;
}

async function trim(cache, max) {
  const keys = await cache.keys();
  for (let i = 0; i < keys.length - max; i++) await cache.delete(keys[i]);
}

self.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.method !== "GET") return;
  const url = new URL(req.url);
  if (url.origin !== self.location.origin) return;
  if (url.pathname.endsWith("/api/live")) return;
  if (url.pathname.includes("/assets/")) {
    event.respondWith(cacheFirst(req));
  } else {
    event.respondWith(networkFirst(req));
  }
});
