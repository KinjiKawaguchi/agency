import Foundation

/// The Cloudflare Access team that guards the home node, learned from the node itself.
///
/// A request to the node without a sign-in is answered with a redirect to its login page on the
/// team's domain (`https://<team>.cloudflareaccess.com/cdn-cgi/access/login/...`). That host is the
/// only Access host the web view may show: the page of any other team is a stranger's page, however
/// much it looks like a login.
public enum AccessTeam {
    /// The team host in the `Location` of the node's redirect, or `nil` when the redirect is not to an
    /// Access login page of one team (https, the default port, no user name, a host of the form
    /// `<team>.cloudflareaccess.com`).
    public static func host(fromRedirect location: String) -> String? {
        guard let url = URL(string: location), url.scheme?.lowercased() == "https", url.user == nil,
              (url.port ?? 443) == 443, url.path.hasPrefix(NavigationPolicy.accessLoginPath),
              let host = url.host?.lowercased()
        else { return nil }
        return isTeamHost(host) ? host : nil
    }

    /// `<team>.cloudflareaccess.com` with a team name that is one plain label.
    public static func isTeamHost(_ host: String) -> Bool {
        let suffix = NavigationPolicy.accessDomainSuffix
        guard host.hasSuffix(suffix) else { return false }
        let team = host.dropLast(suffix.count)
        let allowed = Set("abcdefghijklmnopqrstuvwxyz0123456789-".unicodeScalars)
        return !team.isEmpty && !team.hasPrefix("-") && !team.hasSuffix("-") && team.unicodeScalars.allSatisfy(allowed.contains)
    }
}
