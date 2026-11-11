import SwiftUI

@main
struct LeakApp: App {
    var body: some Scene {
        WindowGroup {
            NewsWebView(url: AppConfig.siteURL)
                .ignoresSafeArea(edges: .bottom)
                .background(Color(red: 0.949, green: 0.957, blue: 0.965))
        }
    }
}

enum AppConfig {
    /// The live Leak site. scripts/configure.sh rewrites this for forks.
    static let siteURL = URL(string: "https://mohammedanasasif.github.io/Leak/")!
}
