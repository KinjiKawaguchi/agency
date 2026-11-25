import AgencyKit
import Observation
import UIKit
import WebKit

struct LoadFailure: Equatable {
    let message: String
    /// The page to try again, or `nil` for the home node's front page.
    let url: URL?
}

/// Owns the web view that shows the home node.
///
/// The web view uses the default website data store, which is persistent: the Cloudflare Access
/// session cookie and the web client's own storage survive app launches.
@MainActor
@Observable
final class NodeWebModel: NSObject {
    let webView: WKWebView
    private(set) var failure: LoadFailure?

    @ObservationIgnored private let homeNode: HomeNodeURL
    @ObservationIgnored private let policy: NavigationPolicy

    init(homeNode: HomeNodeURL) {
        self.homeNode = homeNode
        policy = NavigationPolicy(homeNode: homeNode)

        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .default()
        configuration.applicationNameForUserAgent = "AgencyApp"
        webView = WKWebView(frame: .zero, configuration: configuration)
        super.init()

        webView.navigationDelegate = self
        webView.uiDelegate = self
        webView.allowsBackForwardNavigationGestures = true
        webView.isOpaque = false
        webView.backgroundColor = .systemBackground
        webView.scrollView.backgroundColor = .systemBackground
        #if DEBUG
        webView.isInspectable = true
        #endif

        let refreshControl = UIRefreshControl()
        refreshControl.addTarget(self, action: #selector(pulledToRefresh), for: .valueChanged)
        webView.scrollView.refreshControl = refreshControl

        webView.load(URLRequest(url: homeNode.url))
    }

    /// Loads again the page that failed, or the front page when none did.
    func retry() {
        let target = failure?.url ?? homeNode.url
        failure = nil
        webView.load(URLRequest(url: target))
    }

    @objc private func pulledToRefresh() {
        if failure != nil {
            retry()
        } else if webView.url == nil {
            webView.load(URLRequest(url: homeNode.url))
        } else {
            webView.reload()
        }
    }

    fileprivate func finishRefreshing() {
        webView.scrollView.refreshControl?.endRefreshing()
    }

    fileprivate func fail(_ error: Error) {
        finishRefreshing()
        let nsError = error as NSError
        if Self.isCancellation(nsError) { return }
        let url = nsError.userInfo[NSURLErrorFailingURLErrorKey] as? URL
        failure = LoadFailure(message: nsError.localizedDescription, url: url)
    }

    /// A navigation that the app or the page stopped itself is not a failure to report.
    private static func isCancellation(_ error: NSError) -> Bool {
        let isUrlCancel = error.domain == NSURLErrorDomain && error.code == NSURLErrorCancelled
        let isPolicyStop = error.domain == "WebKitErrorDomain" && error.code == 102
        return isUrlCancel || isPolicyStop
    }

    /// Hands the address to the system without waiting for the result.
    private func openExternally(_ url: URL) {
        UIApplication.shared.open(url, options: [:], completionHandler: nil)
    }

    private func handleLink(to url: URL) {
        switch policy.decide(for: url) {
        case .allowInApp: webView.load(URLRequest(url: url))
        case .openExternally: openExternally(url)
        case .block: break
        }
    }
}

extension NodeWebModel: WKNavigationDelegate {
    func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction
    ) async -> WKNavigationActionPolicy {
        let isMainFrame = navigationAction.targetFrame?.isMainFrame ?? true
        guard isMainFrame, let url = navigationAction.request.url else { return .allow }
        switch policy.decide(for: url) {
        case .allowInApp:
            return .allow
        case .openExternally:
            openExternally(url)
            return .cancel
        case .block:
            return .cancel
        }
    }

    /// An error page from the node itself would leave the owner with no way out, so the app shows
    /// its own page with Reload and Change home node.
    func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationResponse: WKNavigationResponse
    ) async -> WKNavigationResponsePolicy {
        guard navigationResponse.isForMainFrame,
              let response = navigationResponse.response as? HTTPURLResponse,
              response.statusCode >= 400,
              let url = response.url,
              url.host?.lowercased() == homeNode.host
        else { return .allow }
        failure = LoadFailure(
            message: "The node answered with an error (HTTP \(response.statusCode)).",
            url: homeNode.url
        )
        finishRefreshing()
        return .cancel
    }

    func webView(_ webView: WKWebView, didCommit navigation: WKNavigation!) {
        failure = nil
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        finishRefreshing()
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        fail(error)
    }

    func webView(
        _ webView: WKWebView,
        didFailProvisionalNavigation navigation: WKNavigation!,
        withError error: Error
    ) {
        fail(error)
    }

    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        webView.reload()
    }
}

extension NodeWebModel: WKUIDelegate {
    /// Links that ask for a new window (`target="_blank"`) never open one: pages the app trusts load
    /// in this web view, everything else goes to the system.
    func webView(
        _ webView: WKWebView,
        createWebViewWith configuration: WKWebViewConfiguration,
        for navigationAction: WKNavigationAction,
        windowFeatures: WKWindowFeatures
    ) -> WKWebView? {
        if let url = navigationAction.request.url { handleLink(to: url) }
        return nil
    }
}
