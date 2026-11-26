import AgencyKit
import Foundation

/// The link a widget carries to open a request in the app: `agency://request?org=..&team=..&request=..`.
///
/// It only says where to go. The app opens the desk at a request that is already on it, and the three values must
/// look like identifiers, so nothing in a link can change which page is opened or do anything else.
public enum WidgetLink {
    public static let scheme = "agency"
    static let host = "request"

    public static func url(for link: RequestLink) -> URL {
        var components = URLComponents()
        components.scheme = scheme
        components.host = host
        components.queryItems = [
            URLQueryItem(name: "org", value: link.org),
            URLQueryItem(name: "team", value: link.team),
            URLQueryItem(name: "request", value: link.request),
        ]
        return components.url ?? URL(fileURLWithPath: "/")
    }

    /// The request a link names, or `nil` for any other URL, including one with a value that is not an identifier.
    public static func request(from url: URL) -> RequestLink? {
        guard url.scheme == scheme, url.host == host,
              let items = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems
        else { return nil }
        let values = Dictionary(items.compactMap { item in item.value.map { (item.name, $0) } }, uniquingKeysWith: { first, _ in first })
        // Exactly three values, and each of ours present: a fourth, or one given twice, is not one of ours.
        guard items.count == 3, let org = values["org"], let team = values["team"], let request = values["request"] else { return nil }
        return RequestLink(org: org, team: team, request: request)
    }
}
