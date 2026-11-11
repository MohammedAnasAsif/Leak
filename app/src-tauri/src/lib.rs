//! Leak native shell for Windows, macOS, Linux, Android and iOS.
//!
//! The UI is the same `web/` folder the website serves. It talks to the Leak
//! news server over HTTPS (REST + a live Server-Sent Events stream), so new
//! stories arrive on every device the moment the engine detects them.
//! Native extras: system notifications for breaking news, and publisher
//! links opening in the system browser.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .run(tauri::generate_context!())
        .expect("error while running Leak");
}
