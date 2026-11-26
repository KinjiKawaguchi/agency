import AgencyAPI
import AgencyKit
import Foundation

/// The home nodes this device knows: the current one, the ones the owner added on this device, and what the
/// current node last said about the owner's other nodes. A value; the app keeps one and writes it (`encoded`).
///
/// Everything the app keeps for a node (the Access token, the device key, the drafts, the choices) is keyed by the node's
/// host and is not in here: switching changes which node is current and moves no secret.
public struct HomeNodeBook: Equatable, Sendable {
    /// The node the app is showing, or none before the first launch's address and after leaving it.
    public private(set) var current: HomeNodeURL?
    /// The nodes the owner typed on this device, in the order they were added, without the current one and without
    /// duplicates.
    public private(set) var added: [HomeNodeEntry]
    /// What the current node last said about the owner's other nodes (`Me.homeNodes`), kept so the list shows offline.
    public private(set) var configured: [HomeNodeRef]
    /// The id each node said it uses in its pushes (`Me.nodeId`), by the origin the app reaches it at. A node reached at one address
    /// and configured with another names itself by its own, so what it said is what a push is mapped by; a node that has not said
    /// is mapped by the id worked out from its address (`nodeID(of:)`).
    public private(set) var nodeIDs: [String: String]
    /// The origins of the nodes the owner left: they are on the list but signed out, so a late push of theirs (their notifications
    /// could not be stopped) does not put a count back on the icon. A node is signed in again when it becomes the open one.
    public private(set) var signedOut: Set<String>

    public init(
        current: HomeNodeURL? = nil, added: [HomeNodeEntry] = [], configured: [HomeNodeRef] = [], nodeIDs: [String: String] = [:],
        signedOut: Set<String> = []
    ) {
        self.current = current
        self.configured = configured
        self.signedOut = signedOut
        self.nodeIDs = nodeIDs.filter { NotificationTap.isNodeID($0.value) }
        self.added = []
        for entry in added where entry.source == .added { remember(entry) }
        if let current { forgetEntry(of: current) }
    }

    /// The nodes to offer besides the current one (the web's rule, `HomeNodeList`).
    public var others: [HomeNodeEntry] {
        guard let current else { return added }
        return HomeNodeList.others(current: current, configured: configured, added: added)
    }

    /// The nodes this device is signed in to as far as the book knows: the open one and the ones on the list that were not left.
    /// Their pushes count on the icon.
    public var activeNodes: [HomeNodeURL] {
        let listed = others.map(\.url).filter { !signedOut.contains($0.origin) }
        return (current.map { [$0] } ?? []) + listed
    }

    // MARK: Changes

    /// The owner added a node to the list without going to it. The current node is not listed twice.
    public mutating func add(_ node: HomeNodeURL, label: String) {
        guard !node.sameNode(as: current) else { return }
        remember(HomeNodeEntry(url: node, label: label, source: .added))
    }

    /// Go to a node (one on the list, or an address the owner just entered). The one that was current stays on the list,
    /// because the owner went there once.
    public mutating func switchTo(_ node: HomeNodeURL) {
        if let previous = current, !previous.sameNode(as: node) {
            remember(HomeNodeEntry(url: previous, label: previous.host, source: .added))
        }
        forgetEntry(of: node)
        signedOut.remove(node.origin)
        if !node.sameNode(as: current) { configured = [] }
        current = node
    }

    /// Leave the current node: nothing is current (the setup asks for an address), and the node stays on the list as one the
    /// owner went to, so that it can still be forgotten, and what the device kept for it is not left without a row.
    public mutating func leaveCurrent() {
        if let previous = current {
            remember(HomeNodeEntry(url: previous, label: previous.host, source: .added))
            signedOut.insert(previous.origin)
        }
        current = nil
        configured = []
    }

    /// Take a node the owner added off this device's list. Returns whether what the device kept for the node's host (its
    /// sign-in, key, drafts and choices, all keyed by host alone) may go too: not when the node is the current one or is not an
    /// added one (nothing changes then; a node the current node names comes back from it), and not when another node the book knows (the current one, an added one, one the current node names)
    /// has the same host with another scheme or port.
    @discardableResult
    public mutating func forget(_ node: HomeNodeURL) -> Bool {
        // The current node is never in `added`, so this also refuses it.
        guard added.contains(where: { $0.url.sameNode(as: node) }) else { return false }
        forgetEntry(of: node)
        nodeIDs[node.origin] = nil
        signedOut.remove(node.origin)
        return !hostIsUsed(by: node)
    }

    /// Whether a node other than `node` that the book knows has the same host.
    private func hostIsUsed(by node: HomeNodeURL) -> Bool {
        let configuredNodes = configured.compactMap { try? HomeNodeURL(validating: $0.url) }
        let known = (current.map { [$0] } ?? []) + added.map(\.url) + configuredNodes
        return known.contains { !$0.sameNode(as: node) && $0.host == node.host }
    }

    /// What a node said of its own id in `/api/me`: kept for the node (by origin) when it is an id, and forgotten when the node
    /// says none or something that is not one.
    public mutating func note(nodeID: String?, of node: HomeNodeURL) {
        if let nodeID, NotificationTap.isNodeID(nodeID) { nodeIDs[node.origin] = nodeID } else { nodeIDs[node.origin] = nil }
    }

    /// Whether `node` has said its id, so that `nodeID(of:)` is its word and not worked out from its address.
    public func hasSaidID(of node: HomeNodeURL) -> Bool { nodeIDs[node.origin] != nil }

    /// The id a push from `node` carries: what the node said of it, else the one worked out from its address.
    public func nodeID(of node: HomeNodeURL) -> String {
        nodeIDs[node.origin] ?? NodeID.of(node)
    }

    /// What the current node said about the owner's other nodes.
    public mutating func note(configured refs: [HomeNodeRef]) {
        configured = refs
    }

    private mutating func remember(_ entry: HomeNodeEntry) {
        guard !added.contains(where: { $0.url.sameNode(as: entry.url) }) else { return }
        added.append(HomeNodeEntry(url: entry.url, label: HomeNodeList.label(entry.label, for: entry.url, limit: HomeNodeList.addedLabelLimit), source: .added))
    }

    private mutating func forgetEntry(of node: HomeNodeURL) {
        added.removeAll { $0.url.sameNode(as: node) }
    }
}

extension HomeNodeURL {
    /// Whether two addresses are the same node: scheme, host and port, however they were written.
    func sameNode(as other: HomeNodeURL?) -> Bool {
        guard let other else { return false }
        return HomeNodeList.origin(of: self) == HomeNodeList.origin(of: other)
    }
}

// MARK: Keeping it

extension HomeNodeBook {
    private struct Stored: Codable {
        var current: String?
        var added: [Entry]
        var configured: [Ref]
        var nodeIDs: [String: String]?
        var signedOut: [String]?

        struct Entry: Codable {
            var url: String
            var label: String
        }

        struct Ref: Codable {
            var label: String
            var url: String
        }
    }

    /// The book as text to keep: addresses and labels, no secret.
    public func encoded() -> Data {
        let stored = Stored(
            current: current?.url.absoluteString,
            added: added.map { .init(url: $0.url.url.absoluteString, label: $0.label) },
            configured: configured.map { .init(label: $0.label, url: $0.url) },
            nodeIDs: nodeIDs,
            signedOut: signedOut.sorted()
        )
        return (try? JSONEncoder().encode(stored)) ?? Data()
    }

    /// The book from what `encoded` gave, or from the single address an earlier version kept; an entry that cannot be opened as
    /// a home node any more is dropped. Nothing kept is an empty book.
    public init(decoding data: Data?, legacyAddress: String?) {
        if let data, let stored = try? JSONDecoder().decode(Stored.self, from: data) {
            self.init(
                current: stored.current.flatMap { try? HomeNodeURL(validating: $0) },
                added: stored.added.compactMap { entry in
                    (try? HomeNodeURL(validating: entry.url)).map { HomeNodeEntry(url: $0, label: entry.label, source: .added) }
                },
                configured: stored.configured.map { HomeNodeRef(label: $0.label, url: $0.url) },
                nodeIDs: stored.nodeIDs ?? [:],
                signedOut: Set(stored.signedOut ?? [])
            )
        } else {
            self.init(current: legacyAddress.flatMap { try? HomeNodeURL(validating: $0) })
        }
    }
}
