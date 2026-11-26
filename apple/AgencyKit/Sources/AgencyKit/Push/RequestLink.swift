import Foundation

/// The request a notification is about, from the `agency` keys of its payload.
///
/// ```json
/// { "agency": { "org": "acme", "team": "app", "request": "r-20261124-110000-a1b2" } }
/// ```
public struct RequestLink: Equatable, Sendable {
    public let org: String
    public let team: String
    public let request: String

    /// Returns `nil` unless all three values are present and look like identifiers, so that nothing
    /// in a payload can change which page is opened.
    public init?(userInfo: [AnyHashable: Any]) {
        guard let keys = userInfo["agency"] as? [String: Any],
              let org = Self.identifier(keys["org"]),
              let team = Self.identifier(keys["team"]),
              let request = Self.identifier(keys["request"])
        else { return nil }
        self.org = org
        self.team = team
        self.request = request
    }

    private static let allowedScalars = Set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._-".unicodeScalars)
    private static let maximumLength = 128

    private static func identifier(_ value: Any?) -> String? {
        guard let text = value as? String, !text.isEmpty, text.count <= maximumLength,
              text.unicodeScalars.allSatisfy(allowedScalars.contains)
        else { return nil }
        return text
    }
}
