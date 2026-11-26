import Foundation

/// A tapped notification: where it leads, and which home node sent it.
///
/// Every push of a node that knows its origin names the node in `agency.node` (`docs/protocol.md`, "The node names itself"):
/// 32 lowercase hex characters. A payload without the field, or with one that is not that, names no node, and the tap is
/// taken for the node the app has open.
public struct NotificationTap: Equatable, Sendable {
    public let link: NotificationLink
    /// The id of the node that sent it (`NodeID`), when the push names one.
    public let node: String?
    /// When the app got it: a tap waits for what it needs to be placed, and not longer than `lifetime`.
    public let receivedAt: Date

    /// How long a tap that cannot be placed yet waits.
    public static let lifetime: TimeInterval = 60

    public init(link: NotificationLink, node: String?, receivedAt: Date = Date()) {
        self.link = link
        self.node = node
        self.receivedAt = receivedAt
    }

    /// The org the notification is about, when it names one (a device request names none).
    public var org: String? { link.org }

    /// Whether it has waited longer than `lifetime`.
    public func isStale(now: Date) -> Bool { now.timeIntervalSince(receivedAt) > Self.lifetime }

    /// `nil` when the payload is not ours or has no link (`NotificationLink`).
    public init?(userInfo: [AnyHashable: Any], receivedAt: Date = Date()) {
        guard let link = NotificationLink(userInfo: userInfo) else { return nil }
        self.link = link
        node = Self.nodeID(in: userInfo)
        self.receivedAt = receivedAt
    }

    /// The node's id, when the payload carries one in the shape the node writes.
    public static func nodeID(in userInfo: [AnyHashable: Any]) -> String? {
        guard let keys = userInfo["agency"] as? [String: Any], let id = keys["node"] as? String else { return nil }
        return isNodeID(id) ? id : nil
    }

    /// 32 lowercase hex characters.
    public static func isNodeID(_ text: String) -> Bool {
        text.utf8.count == 32 && text.utf8.allSatisfy { ($0 >= 48 && $0 <= 57) || ($0 >= 97 && $0 <= 102) }
    }
}
