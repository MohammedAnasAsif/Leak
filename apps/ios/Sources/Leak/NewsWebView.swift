import SwiftUI
import UIKit
import WebKit

/// Shows the Leak site. Pages from the site stay in the app; links to outlets
/// open in Safari. Pull down to refresh, swipe from the edge to go back.
struct NewsWebView: UIViewRepresentable {
    let url: URL

    func makeCoordinator() -> Coordinator {
        Coordinator(site: url)
    }

    func makeUIView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        config.limitsNavigationsToAppBoundDomains = true
        config.allowsInlineMediaPlayback = true
        config.userContentController.addUserScript(
            WKUserScript(
                source: "window.__LEAK_APP__ = true;",
                injectionTime: .atDocumentStart,
                forMainFrameOnly: true
            )
        )

        let webView = WKWebView(frame: .zero, configuration: config)
        webView.navigationDelegate = context.coordinator
        webView.allowsBackForwardNavigationGestures = true
        webView.isOpaque = false
        webView.backgroundColor = .clear

        let refresh = UIRefreshControl()
        refresh.addTarget(context.coordinator, action: #selector(Coordinator.pullToRefresh(_:)), for: .valueChanged)
        webView.scrollView.refreshControl = refresh

        context.coordinator.webView = webView
        webView.load(URLRequest(url: url))
        return webView
    }

    func updateUIView(_ uiView: WKWebView, context: Context) {}

    @MainActor
    final class Coordinator: NSObject, WKNavigationDelegate {
        let site: URL
        weak var webView: WKWebView?

        init(site: URL) {
            self.site = site
        }

        @objc func pullToRefresh(_ sender: UIRefreshControl) {
            webView?.reload()
            sender.endRefreshing()
        }

        private func isInternal(_ url: URL) -> Bool {
            if url.scheme == "about" { return true }
            return url.host == site.host && url.path.lowercased().hasPrefix(site.path.lowercased())
        }

        func webView(
            _ webView: WKWebView,
            decidePolicyFor navigationAction: WKNavigationAction,
            decisionHandler: @escaping @MainActor (WKNavigationActionPolicy) -> Void
        ) {
            guard let url = navigationAction.request.url else {
                decisionHandler(.cancel)
                return
            }
            if isInternal(url) {
                decisionHandler(.allow)
            } else {
                UIApplication.shared.open(url)
                decisionHandler(.cancel)
            }
        }

        func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
            let html = """
            <!doctype html><meta name=viewport content="width=device-width,initial-scale=1">
            <body style="margin:0;min-height:100vh;display:grid;place-items:center;background:#0c1626;color:#e8edf3;font:17px -apple-system,sans-serif;text-align:center;padding:2rem">
            <div><h2 style="margin:0 0 .5rem">Leak can't reach the news right now</h2>
            <p style="color:#a9b6c8">Check your connection, then pull down to try again.</p></div></body>
            """
            webView.loadHTMLString(html, baseURL: site)
        }
    }
}
