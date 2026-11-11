/* Leak client: live updates, relative times, freshness, saved stories, search, listen, share.
   No framework, no build step, no tracking. Everything personal stays in localStorage. */
(function () {
  "use strict";

  var body = document.body;
  var root = body.dataset.root || "";
  var KEY = "leak:";
  var store = {
    get: function (k, d) { try { var v = localStorage.getItem(KEY + k); return v === null ? d : JSON.parse(v); } catch (e) { return d; } },
    set: function (k, v) { try { localStorage.setItem(KEY + k, JSON.stringify(v)); } catch (e) {} }
  };
  var $ = function (s, el) { return (el || document).querySelector(s); };
  var $$ = function (s, el) { return Array.prototype.slice.call((el || document).querySelectorAll(s)); };

  /* ── Time ─────────────────────────────────────────────── */
  function rel(date) {
    var s = Math.max(0, (Date.now() - date.getTime()) / 1000);
    if (s < 45) return "just now";
    var m = Math.round(s / 60);
    if (m < 60) return m + " min ago";
    var h = Math.floor(m / 60);
    if (h < 24) return h + " h ago";
    var d = Math.floor(h / 24);
    return d === 1 ? "yesterday" : d + " days ago";
  }
  var clockFmt = new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" });
  var fullFmt = new Intl.DateTimeFormat(undefined, { weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

  function paintTimes() {
    $$("time[datetime]").forEach(function (t) {
      var d = new Date(t.getAttribute("datetime"));
      if (isNaN(d)) return;
      t.title = fullFmt.format(d);
      if (t.closest(".wire__item") || t.hasAttribute("data-clock")) {
        t.textContent = clockFmt.format(d);
      } else if (t.hasAttribute("data-rel")) {
        t.textContent = rel(d);
      }
    });
    // Freshness: the marker fades over 45 minutes.
    $$(".wire__item").forEach(function (li) {
      var age = (Date.now() - new Date(li.dataset.published).getTime()) / 60000;
      var heat = Math.max(0, Math.min(1, 1 - age / 45));
      li.style.setProperty("--heat", heat.toFixed(3));
    });
  }

  /* ── New since your last visit ────────────────────────── */
  function markNew() {
    var last = store.get("lastVisit", 0);
    if (last) {
      $$("[data-story][data-created]").forEach(function (el) {
        if (new Date(el.dataset.created).getTime() > last) {
          var f = $(".flag--new", el);
          if (f) f.hidden = false;
        }
      });
    }
    // Count this visit after a short read, so a quick bounce doesn't clear the marks.
    setTimeout(function () { store.set("lastVisit", Date.now()); }, 8000);
  }

  /* ── Live updates ─────────────────────────────────────── */
  var pageGenerated = new Date(body.dataset.generated || 0).getTime();
  var pill = $(".newpill");
  function showPill(text) {
    if (!pill) return;
    $("[data-newpill-text]", pill).textContent = text;
    pill.hidden = false;
  }
  function checkLatest() {
    if (document.hidden) return;
    var bucket = Math.floor(Date.now() / 30000);
    fetch(root + "api/latest.json?t=" + bucket, { cache: "no-store" })
      .then(function (r) { return r.ok ? r.json() : null; })
      .then(function (data) {
        if (!data) return;
        var gen = new Date(data.generated).getTime();
        if (gen <= pageGenerated) return;
        var fresh = (data.stories || []).filter(function (s) { return new Date(s.created).getTime() > pageGenerated; }).length;
        showPill(fresh > 0 ? "Show " + fresh + " new stor" + (fresh === 1 ? "y" : "ies") : "Show latest updates");
      })
      .catch(function () {});
  }
  function startLive() {
    if (body.dataset.live && "EventSource" in window) {
      var es = new EventSource(body.dataset.live);
      es.addEventListener("update", function (e) {
        try {
          var d = JSON.parse(e.data);
          showPill(d.new_stories > 0 ? "Show " + d.new_stories + " new stor" + (d.new_stories === 1 ? "y" : "ies") : "Show latest updates");
        } catch (err) {}
      });
      return;
    }
    setInterval(checkLatest, 60000);
    document.addEventListener("visibilitychange", function () { if (!document.hidden) checkLatest(); });
  }

  /* ── Saved stories ────────────────────────────────────── */
  function saved() { return store.get("saved", []); }
  function isSaved(id) { return saved().some(function (s) { return s.id === id; }); }
  function toggleSave(story) {
    var list = saved();
    var i = list.findIndex(function (s) { return s.id === story.id; });
    if (i >= 0) list.splice(i, 1); else list.unshift(story);
    store.set("saved", list.slice(0, 500));
    return i < 0;
  }
  function storyPath(path) {
    // Saved paths are site-relative ("story/…"); resolve them from wherever we are.
    return root + path;
  }

  function renderSaved() {
    var ul = $("[data-saved]");
    if (!ul) return;
    var list = saved();
    ul.innerHTML = "";
    $("[data-saved-empty]").hidden = list.length > 0;
    list.forEach(function (s) {
      var li = document.createElement("li");
      var a = document.createElement("a");
      a.className = "res-title"; a.href = storyPath(s.path); a.textContent = s.title;
      var rm = document.createElement("button");
      rm.className = "linkish"; rm.type = "button"; rm.textContent = "Remove";
      rm.addEventListener("click", function () { toggleSave(s); renderSaved(); });
      var meta = document.createElement("p");
      meta.className = "meta";
      meta.innerHTML = "<span></span><span></span>";
      meta.children[0].textContent = s.section || "";
      meta.children[1].textContent = "Saved " + rel(new Date(s.savedAt));
      li.append(a, rm, meta);
      ul.appendChild(li);
    });
  }

  /* ── Listen (on-device text to speech, free and private) ── */
  var synth = window.speechSynthesis;
  function speak(parts, onPart, onEnd) {
    if (!synth) return false;
    synth.cancel();
    parts.forEach(function (text, i) {
      var u = new SpeechSynthesisUtterance(text);
      u.rate = 1.02;
      u.onstart = function () { if (onPart) onPart(i); };
      if (i === parts.length - 1) u.onend = onEnd;
      synth.speak(u);
    });
    return true;
  }
  function setLabel(btn, text) { var l = $("[data-label]", btn); (l || btn).textContent = text; }

  /* ── Story page actions ───────────────────────────────── */
  function initStory() {
    var art = $("article.story");
    if (!art) return;
    var story = { id: art.dataset.story, title: art.dataset.title, path: art.dataset.path, section: art.dataset.section };
    var save = $('[data-action="save"]');
    if (save) {
      var sync = function () {
        var on = isSaved(story.id);
        save.setAttribute("aria-pressed", String(on));
        setLabel(save, on ? "Saved" : "Save");
      };
      sync();
      save.addEventListener("click", function () {
        story.savedAt = Date.now();
        toggleSave(story);
        sync();
      });
    }
    var listen = $('[data-action="listen"]');
    if (listen) {
      if (!synth) { listen.hidden = true; }
      listen.addEventListener("click", function () {
        if (synth.speaking) { synth.cancel(); setLabel(listen, "Listen"); return; }
        var parts = [story.title];
        var dek = $(".story__dek");
        if (dek) parts.push(dek.firstChild.textContent.trim());
        var lede = $(".lede");
        if (lede) parts.push(lede.textContent.trim());
        $$(".report").slice(0, 6).forEach(function (r) {
          parts.push($(".report__src", r).firstChild.textContent.trim() + ": " + $(".report__title", r).textContent.trim());
        });
        setLabel(listen, "Stop");
        speak(parts, null, function () { setLabel(listen, "Listen"); });
      });
    }
    var share = $('[data-action="share"]');
    if (share) {
      share.addEventListener("click", function () {
        var url = location.href.split("#")[0];
        if (navigator.share) {
          navigator.share({ title: story.title, text: story.title, url: url }).catch(function () {});
        } else if (navigator.clipboard) {
          navigator.clipboard.writeText(story.title + " " + url).then(function () {
            share.textContent = "Link copied";
            setTimeout(function () { share.textContent = "Share"; }, 2000);
          });
        }
      });
    }
  }

  function initBrief() {
    var btn = $('[data-action="listen-brief"]');
    if (!btn) return;
    if (!synth) { btn.hidden = true; return; }
    btn.addEventListener("click", function () {
      var items = $$(".brief__item");
      items.forEach(function (i) { i.classList.remove("is-speaking"); });
      if (synth.speaking) { synth.cancel(); setLabel(btn, "Listen to the brief"); return; }
      var parts = ["Here is today's brief from Leak."];
      items.forEach(function (it, n) {
        var t = $(".brief__title", it).textContent.trim();
        var d = $(".brief__dek", it);
        parts.push("Story " + (n + 1) + ". " + t + (d ? ". " + d.textContent.trim() : ""));
      });
      setLabel(btn, "Stop");
      speak(parts, function (i) {
        items.forEach(function (it) { it.classList.remove("is-speaking"); });
        if (i > 0 && items[i - 1]) {
          items[i - 1].classList.add("is-speaking");
          items[i - 1].scrollIntoView({ block: "center", behavior: "smooth" });
        }
      }, function () {
        setLabel(btn, "Listen to the brief");
        items.forEach(function (it) { it.classList.remove("is-speaking"); });
      });
    });
  }

  /* ── Search ───────────────────────────────────────────── */
  function initSearch() {
    var form = $("[data-search]");
    if (!form) return;
    var input = $("input", form), out = $("[data-results]"), status = $("[data-search-status]");
    var index = null;
    function load() {
      if (index) return Promise.resolve(index);
      return fetch(root + "api/search.json", { cache: "no-cache" }).then(function (r) { return r.json(); }).then(function (d) { index = d; return d; });
    }
    function esc(s) { return s.replace(/[&<>"]/g, function (c) { return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]; }); }
    function run(q) {
      q = q.trim();
      history.replaceState(null, "", q ? "?q=" + encodeURIComponent(q) : location.pathname);
      if (!q) { out.innerHTML = ""; return; }
      load().then(function (rows) {
        var terms = q.toLowerCase().split(/\s+/).filter(Boolean);
        var hits = [];
        rows.forEach(function (r) {
          // [title, path, section, updated, tags, sources, first source]
          var hay = (r[0] + " " + r[4] + " " + r[2] + " " + r[6]).toLowerCase();
          var score = 0;
          for (var i = 0; i < terms.length; i++) {
            if (hay.indexOf(terms[i]) < 0) return;
            score += r[0].toLowerCase().indexOf(terms[i]) >= 0 ? 3 : 1;
          }
          score += Math.log(1 + r[5]) + new Date(r[3]).getTime() / 864e8 / 1000;
          hits.push([score, r]);
        });
        hits.sort(function (a, b) { return b[0] - a[0]; });
        status.textContent = hits.length ? hits.length + " stor" + (hits.length === 1 ? "y" : "ies") + " match “" + q + "”" : "No stories match “" + q + "” in the last week. Try a shorter or different word.";
        var re = new RegExp("(" + terms.map(function (t) { return t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"); }).join("|") + ")", "ig");
        out.innerHTML = hits.slice(0, 80).map(function (h) {
          var r = h[1];
          return '<li><a class="res-title" href="' + esc(root + r[1]) + '">' + esc(r[0]).replace(re, "<mark>$1</mark>") + "</a>" +
            '<p class="meta"><span>' + esc(r[2]) + "</span><span>" + r[5] + " outlet" + (r[5] === 1 ? "" : "s") + "</span><span>" + rel(new Date(r[3])) + "</span></p></li>";
        }).join("");
      }).catch(function () { status.textContent = "Search needs a connection the first time. Reconnect and try again."; });
    }
    form.addEventListener("submit", function (e) { e.preventDefault(); run(input.value); });
    var t;
    input.addEventListener("input", function () { clearTimeout(t); t = setTimeout(function () { run(input.value); }, 160); });
    var q = new URLSearchParams(location.search).get("q");
    if (q) { input.value = q; run(q); }
  }

  /* ── Theme and text size ──────────────────────────────── */
  function initTheme() {
    var btn = $('[data-action="theme"]');
    if (!btn) return;
    var order = ["system", "light", "dark"];
    btn.addEventListener("click", function () {
      var cur = store.get("theme_mode", "system");
      var next = order[(order.indexOf(cur) + 1) % order.length];
      store.set("theme_mode", next);
      if (next === "system") {
        delete document.documentElement.dataset.theme;
        try { localStorage.removeItem("leak:theme"); } catch (e) {}
      } else {
        document.documentElement.dataset.theme = next;
        try { localStorage.setItem("leak:theme", next); } catch (e) {}
      }
      btn.title = "Theme: " + next;
      btn.setAttribute("aria-label", "Theme: " + next + ". Change theme");
    });
  }

  /* ── Keyboard ─────────────────────────────────────────── */
  function initKeys() {
    document.addEventListener("keydown", function (e) {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      var tag = (e.target.tagName || "").toLowerCase();
      if (tag === "input" || tag === "textarea") return;
      if (e.key === "/") {
        e.preventDefault();
        var s = $("#q");
        if (s) s.focus(); else location.href = root + "search.html";
      } else if (e.key === "j" || e.key === "k") {
        var links = $$(".lead__title a, .card__title a, .wire__item a, .brief__title a");
        var i = links.indexOf(document.activeElement);
        var n = e.key === "j" ? Math.min(links.length - 1, i + 1) : Math.max(0, i - 1);
        if (links[n]) { links[n].focus(); links[n].scrollIntoView({ block: "center" }); }
      }
    });
  }

  /* ── Inside the Android/Windows/iOS apps ──────────────── */
  function initApp() {
    if (!window.__LEAK_APP__) return;
    // Outlet links open in the system browser (the app intercepts the navigation).
    $$('a[target="_blank"]').forEach(function (a) { a.removeAttribute("target"); });
  }

  /* ── Offline support ──────────────────────────────────── */
  function initSW() {
    if (!("serviceWorker" in navigator) || location.protocol === "file:") return;
    navigator.serviceWorker.register(root + "sw.js").catch(function () {});
  }

  document.addEventListener("click", function (e) {
    var b = e.target.closest('[data-action="refresh"]');
    if (b) location.reload();
    var w = e.target.closest('[data-action="wire-all"]');
    if (w) w.closest(".wire").classList.add("is-open");
  });

  paintTimes();
  setInterval(paintTimes, 30000);
  markNew();
  startLive();
  renderSaved();
  initStory();
  initBrief();
  initSearch();
  initTheme();
  initKeys();
  initApp();
  initSW();
})();
