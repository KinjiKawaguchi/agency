import AgencyKit
import Foundation

/// Which home node a tapped notification belongs to, against the nodes this device has.
public enum NotificationRouting {
    public enum Action: Equatable, Sendable {
        /// The node that is open, or a push that names none (a node that does not know its origin): open it here.
        case openHere
        /// A node on this device that is not the open one: go to it, then open the notification there.
        case switchTo(HomeNodeURL)
        /// A node this device does not have (forgotten, or never added): open nothing of it.
        case unknownNode
    }

    /// - Parameters:
    ///   - node: the id the push names (`NotificationTap.node`).
    ///   - current: the node that is open.
    ///   - others: the other nodes on this device.
    ///   - idOf: the id a push from a node carries (`HomeNodeBook.nodeID(of:)`: what the node said, else worked out from its address).
    public static func action(forNode node: String?, current: HomeNodeURL, others: [HomeNodeURL], idOf: (HomeNodeURL) -> String = NodeID.of) -> Action {
        guard let node, node != idOf(current) else { return .openHere }
        if let other = others.first(where: { idOf($0) == node }) { return .switchTo(other) }
        return .unknownNode
    }
}
