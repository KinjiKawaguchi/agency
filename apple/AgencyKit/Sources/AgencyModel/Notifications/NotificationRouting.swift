import AgencyKit
import Foundation

/// Which home node a tapped notification belongs to, against the nodes this device has.
public enum NotificationRouting {
    public enum Action: Equatable, Sendable {
        /// The node that is open, or a push that names none (a node that does not know its origin): open it here.
        case openHere
        /// A node on this device that is not the open one: go to it, then open the notification there.
        case switchTo(HomeNodeURL)
        /// The open node has not yet said its own id (it is only worked out from the address typed), so a push of an id that is
        /// none of the others' may still be its own: wait for the account read, and place the tap then.
        case waitForAccount
        /// A node this device does not have (forgotten, or never added): open nothing of it.
        case unknownNode
    }

    /// - Parameters:
    ///   - node: the id the push names (`NotificationTap.node`).
    ///   - current: the node that is open.
    ///   - others: the other nodes on this device.
    ///   - idOf: the id a push from a node carries (`HomeNodeBook.nodeID(of:)`: what the node said, else worked out from its address).
    ///   - hasCurrentSaid: whether the open node has said its own id (`Me.nodeId`), so that `idOf(current)` is the node's word and not a guess.
    ///   - isAccountRead: whether the account read has finished, with an answer or without: a node that has not said its id by then
    ///     never will (an older node, or a read that failed), so a push that is none of the others' is a stranger's.
    public static func action(
        forNode node: String?, current: HomeNodeURL, others: [HomeNodeURL],
        idOf: (HomeNodeURL) -> String = NodeID.of, hasCurrentSaid: Bool = true, isAccountRead: Bool = false
    ) -> Action {
        guard let node, node != idOf(current) else { return .openHere }
        if let other = others.first(where: { idOf($0) == node }) { return .switchTo(other) }
        return hasCurrentSaid || isAccountRead ? .unknownNode : .waitForAccount
    }
}
