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
/// Only the home node and the login pages of the node's own Cloudflare Access team stay in the app.
/// Every other web address opens in Safari, so a link inside a message can never show a foreign page
/// (another Access team's included) in a view that holds the owner's session. Until the team is
/// known (`AccessTeam`), no Access page stays in the app.
public struct NavigationPolicy: Equatable, Sendable {
    public static let accessDomainSuffix = ".cloudflareaccess.com"
    public static let accessLoginPath = "/cdn-cgi/access/login"

    private static let externalSchemes: Set<String> = ["http", "https", "mailto", "tel"]

    public let homeNode: HomeNodeURL
    /// `<team>.cloudflareaccess.com` of the node's Access team, once learned.
    public let accessTeamHost: String?

    /// - Parameter accessTeamHost: the node's Access team host. A value that is not a team host is ignored.
    public init(homeNode: HomeNodeURL, accessTeamHost: String? = nil) {
        self.homeNode = homeNode
        let host = accessTeamHost?.lowercased()
        self.accessTeamHost = host.flatMap { AccessTeam.isTeamHost($0) ? $0 : nil }
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

    /// The node's own team domain, where sign-in happens before Access sends the browser back to the node.
    private func isAccessLogin(scheme: String, host: String, port: Int) -> Bool {
        scheme == "https" && port == 443 && host == accessTeamHost
    }

    /// The page Access sends a signed-out browser to (`/cdn-cgi/access/login/<node host>`), as
    /// opposed to the other pages on its domain, such as the one that confirms a sign-out.
    public func isAccessLoginPage(_ url: URL) -> Bool {
        guard url.scheme?.lowercased() == "https", let host = url.host?.lowercased() else { return false }
        return isAccessLogin(scheme: "https", host: host, port: url.port ?? 443)
            && url.path.hasPrefix(Self.accessLoginPath)
    }
}
