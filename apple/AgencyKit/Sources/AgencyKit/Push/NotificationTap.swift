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

    public init(link: NotificationLink, node: String?) {
        self.link = link
        self.node = node
    }

    /// `nil` when the payload is not ours or has no link (`NotificationLink`).
    public init?(userInfo: [AnyHashable: Any]) {
        guard let link = NotificationLink(userInfo: userInfo) else { return nil }
        self.link = link
        node = Self.nodeID(in: userInfo)
    }

    /// The node's id, when the payload carries one in the shape the node writes.
    static func nodeID(in userInfo: [AnyHashable: Any]) -> String? {
        guard let keys = userInfo["agency"] as? [String: Any], let id = keys["node"] as? String else { return nil }
        return isNodeID(id) ? id : nil
    }

    /// 32 lowercase hex characters.
    public static func isNodeID(_ text: String) -> Bool {
        text.utf8.count == 32 && text.utf8.allSatisfy { ($0 >= 48 && $0 <= 57) || ($0 >= 97 && $0 <= 102) }
    }
}
