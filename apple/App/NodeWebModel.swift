import AgencyKit
import Observation
import UIKit
import WebKit

struct LoadFailure: Equatable {
    var title = "Cannot reach your home node"
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
    @ObservationIgnored private let userContentController = WKUserContentController()
    @ObservationIgnored private let accessSignIn: AccessSignInFlow
    /// The last page of the home node the web view was sent to, to come back to after signing in.
    @ObservationIgnored private var lastHomePage: URL?

    /// Called each time a page of the home node has loaded. Such a page only loads once the owner has
    /// signed in through Access.
    @ObservationIgnored var onHomeNodeReached: (() -> Void)?

    /// Called with the number of requests waiting for the owner, when the page says so.
    @ObservationIgnored var onBadgeChange: ((Int) -> Void)?

    /// - Parameters:
    ///   - push: this device's registration, for the page to hand to the node.
    ///   - accessSignIn: the one sign-in the whole app shares.
    ///   - path: the page to open first, a path of the home node.
    init(homeNode: HomeNodeURL, push: PushRegistration?, accessSignIn: AccessSignInFlow, path: String) {
        self.homeNode = homeNode
        policy = NavigationPolicy(homeNode: homeNode)
        self.accessSignIn = accessSignIn

        userContentController.addUserScript(Self.bridgeScript(homeNode: homeNode, push: push))
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .default()
        configuration.userContentController = userContentController
        configuration.applicationNameForUserAgent = "AgencyApp"
        webView = WKWebView(frame: .zero, configuration: configuration)
        super.init()

        userContentController.add(ScriptMessageRelay(model: self), name: NativeMessage.handlerName)
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

        webView.load(URLRequest(url: Self.url(of: path, on: homeNode)))
    }

    /// A path of the home node, never another address.
    private static func url(of path: String, on homeNode: HomeNodeURL) -> URL {
        guard path.hasPrefix("/"), !path.hasPrefix("//"), let url = URL(string: path, relativeTo: homeNode.url)?.absoluteURL,
              homeNode.hasSameOrigin(url)
        else { return homeNode.url }
        return url
    }

    /// Shows another page of the home node, unless it is already the one shown.
    func openPath(_ path: String) {
        let url = Self.url(of: path, on: homeNode)
        guard webView.url?.path != url.path || failure != nil else { return }
        failure = nil
        webView.load(URLRequest(url: url))
    }

    /// Tells the page this device's push registration, now and on every page the web view loads next.
    func updatePush(_ push: PushRegistration?) {
        userContentController.removeAllUserScripts()
        userContentController.addUserScript(Self.bridgeScript(homeNode: homeNode, push: push))
        guard let url = webView.url, homeNode.hasSameOrigin(url) else { return }
        let announcement = NativeBridge.script(homeNode: homeNode, push: push, announcesChange: true)
        webView.evaluateJavaScript(announcement, completionHandler: nil)
    }

    /// A message from the page. The page is trusted only as far as its origin: the main frame of the
    /// home node, and a message that is exactly one the app knows.
    fileprivate func receive(_ message: WKScriptMessage) {
        let origin = message.frameInfo.securityOrigin
        guard message.frameInfo.isMainFrame,
              homeNode.hasSameOrigin(scheme: origin.protocol, host: origin.host, port: origin.port),
              let native = NativeMessage(body: message.body)
        else { return }
        switch native {
        case .badge(let count): onBadgeChange?(count)
        }
    }

    private static func bridgeScript(homeNode: HomeNodeURL, push: PushRegistration?) -> WKUserScript {
        WKUserScript(
            source: NativeBridge.script(homeNode: homeNode, push: push, announcesChange: false),
            injectionTime: .atDocumentStart,
            forMainFrameOnly: true
        )
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

    /// Whether the app takes over a login page: the sign-in runs in the system session and the page
    /// stays out of the web view.
    private func startsAccessSignIn(for url: URL) -> Bool {
        guard policy.isAccessLoginPage(url) else { return false }
        switch accessSignIn.next() {
        case .loadInWebView:
            return false
        case .wait:
            return true
        case .start:
            Task { await signInThroughSession(fallingBackTo: url) }
            return true
        }
    }

    private func signInThroughSession(fallingBackTo loginPage: URL) async {
        switch await accessSignIn.run() {
        case .signedIn:
            failure = nil
            webView.load(URLRequest(url: lastHomePage ?? homeNode.url))
        case .canceled:
            failure = LoadFailure(
                title: "Sign-in canceled",
                message: "Reload to sign in again.",
                url: homeNode.url
            )
            finishRefreshing()
        case .unavailable:
            webView.load(URLRequest(url: loginPage))
        }
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

/// The web view's content controller keeps its handlers alive, so the handler holds the model weakly.
@MainActor
private final class ScriptMessageRelay: NSObject, WKScriptMessageHandler {
    private weak var model: NodeWebModel?

    init(model: NodeWebModel) {
        self.model = model
    }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        model?.receive(message)
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
            if homeNode.hasSameOrigin(url) { lastHomePage = url }
            return startsAccessSignIn(for: url) ? .cancel : .allow
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
        if let url = webView.url, homeNode.hasSameOrigin(url) { onHomeNodeReached?() }
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
