//! Leak app shell for Windows and Android.
//!
//! The app opens the live Leak website in a native webview, so every build of the
//! app shows the latest design and features without reinstalling. Links to outlets
//! leave the app and open in the system browser. The site's service worker gives
//! offline reading for anything already opened.

use tauri::{WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;

/// Set at build time with LEAK_SITE_URL; CI passes the GitHub Pages address.
const SITE_URL: &str = match option_env!("LEAK_SITE_URL") {
    Some(url) => url,
    None => "https://mohammedanasasif.github.io/Leak/",
};

fn is_internal(url: &tauri::Url, site_host: &str, site_path: &str) -> bool {
    matches!(url.scheme(), "tauri" | "asset" | "about" | "data" | "blob")
        || url.host_str() == Some("tauri.localhost")
        || (url.host_str() == Some(site_host) && url.path().to_lowercase().starts_with(site_path))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let site: tauri::Url = SITE_URL.parse().expect("LEAK_SITE_URL must be a valid URL");
            let site_host = site.host_str().unwrap_or_default().to_string();
            let site_path = site.path().to_lowercase();
            let handle = app.handle().clone();

            let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(site.clone()))
                // Lets the site know it is inside the app (e.g. to drop target=_blank).
                .initialization_script("window.__LEAK_APP__ = true;")
                .on_navigation(move |url| {
                    if is_internal(url, &site_host, &site_path) {
                        return true;
                    }
                    let _ = handle.opener().open_url(url.as_str(), None::<&str>);
                    false
                });

            #[cfg(desktop)]
            let builder = builder
                .title("Leak")
                .inner_size(1280.0, 860.0)
                .min_inner_size(380.0, 560.0);

            builder.build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Leak");
}
