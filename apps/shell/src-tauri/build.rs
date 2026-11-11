fn main() {
    // Rebuild when the site URL changes (CI sets it to the GitHub Pages address).
    println!("cargo:rerun-if-env-changed=LEAK_SITE_URL");
    tauri_build::build()
}
