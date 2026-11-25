import Foundation

/// What the app does with a page the web view is about to open.
public enum NavigationDecision: Equatable, Sendable {
    /// Load it in the web view.
    case allowInApp
    /// Hand it to the system (Safari, Mail, Phone).
    case openExternally
    /// Do nothing.
    case block
}

/// Decides which pages may load inside the web view, which carries the owner's sign-in.
///
/// Only the home node and the Cloudflare Access login pages stay in the app. Every other web
/// address opens in Safari, so a link inside a message can never show a foreign page in a view that
/// holds the owner's session.
public struct NavigationPolicy: Equatable, Sendable {
    public static let accessDomainSuffix = ".cloudflareaccess.com"

    private static let externalSchemes: Set<String> = ["http", "https", "mailto", "tel"]

    public let homeNode: HomeNodeURL

    public init(homeNode: HomeNodeURL) {
        self.homeNode = homeNode
    }

    public func decide(for url: URL) -> NavigationDecision {
        guard let scheme = url.scheme?.lowercased() else { return .block }
        if url.absoluteString == "about:blank" { return .allowInApp }
        guard scheme == "http" || scheme == "https" else {
            return Self.externalSchemes.contains(scheme) ? .openExternally : .block
        }
        guard let host = url.host?.lowercased(), !host.isEmpty else { return .block }
        let port = url.port ?? HomeNodeURL.defaultPort(forScheme: scheme)

        if homeNode.hasSameOrigin(url) || isAccessLogin(scheme: scheme, host: host, port: port) {
            return .allowInApp
        }
        return .openExternally
    }

    /// `<team>.cloudflareaccess.com`, where one-time PIN sign-in happens, before it sends the
    /// browser back to the node.
    private func isAccessLogin(scheme: String, host: String, port: Int) -> Bool {
        scheme == "https" && port == 443 && host.hasSuffix(Self.accessDomainSuffix)
    }
}
