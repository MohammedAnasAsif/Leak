// Where the clients find the Leak news server.
//  - Website: leave apiBase empty; the page is served by the server itself.
//  - Apps (Windows, Android, iOS): CI rewrites this file at build time from the
//    LEAK_API_URL repository variable, e.g. "https://leak-news.fly.dev".
//    Users can also override it in the app's Settings.
window.LEAK_CONFIG = {
  apiBase: "",
};
