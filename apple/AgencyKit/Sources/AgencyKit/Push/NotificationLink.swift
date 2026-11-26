import Foundation

/// Where a tapped notification leads, from the `agency` keys of its payload.
///
/// A request's payload has no `type` (`{ "agency": { "org", "team", "request" } }`); the others name it:
/// `{ "agency": { "type": "device" } }`, `{ "agency": { "type": "decision", "org", "team", "id" } }` and
/// `{ "agency": { "type": "recovery", "org" } }`.
public enum NotificationLink: Equatable, Sendable {
    case request(RequestLink)
    /// A device asks to be added: the devices screen.
    case devices
    /// A decision was refused: the desk, where its team's requests are.
    case decision(org: String, team: String, id: String)
    /// A recovery of an org was started: the desk, where it is said at the top.
    case recovery(org: String)

    /// The org the notification is about, when it names one.
    public var org: String? {
        switch self {
        case .request(let link): link.org
        case .decision(let org, _, _): org
        case .devices: nil
        }
    }

    /// Returns `nil` for a payload that is not ours, a type this app does not know, or an identifier that
    /// is missing or odd, so that nothing in a payload can change which screen is opened.
    public init?(userInfo: [AnyHashable: Any]) {
        guard let keys = userInfo["agency"] as? [String: Any] else { return nil }
        switch keys["type"] {
        case nil:
            guard let link = RequestLink(userInfo: userInfo) else { return nil }
            self = .request(link)
        case let type as String where type == "device":
            self = .devices
        case let type as String where type == "decision":
            guard let org = RequestLink.identifier(keys["org"]), let team = RequestLink.identifier(keys["team"]),
                  let id = RequestLink.identifier(keys["id"])
            else { return nil }
            self = .decision(org: org, team: team, id: id)
        case let type as String where type == "recovery":
            guard let org = RequestLink.identifier(keys["org"]) else { return nil }
            self = .recovery(org: org)
        default:
            return nil
        }
    }
}
