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

    public init(current: HomeNodeURL? = nil, added: [HomeNodeEntry] = [], configured: [HomeNodeRef] = []) {
        self.current = current
        self.configured = configured
        self.added = []
        for entry in added where entry.source == .added { remember(entry) }
        if let current { forgetEntry(of: current) }
    }

    /// The nodes to offer besides the current one (the web's rule, `HomeNodeList`).
    public var others: [HomeNodeEntry] {
        guard let current else { return added }
        return HomeNodeList.others(current: current, configured: configured, added: added)
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
        if !node.sameNode(as: current) { configured = [] }
        current = node
    }

    /// Leave the current node: nothing is current (the setup asks for an address) and it is not kept on the list.
    public mutating func leaveCurrent() {
        current = nil
        configured = []
    }

    /// Take a node off this device's list. The current node is left alone (leave it first).
    public mutating func forget(_ node: HomeNodeURL) {
        forgetEntry(of: node)
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
            configured: configured.map { .init(label: $0.label, url: $0.url) }
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
                configured: stored.configured.map { HomeNodeRef(label: $0.label, url: $0.url) }
            )
        } else {
            self.init(current: legacyAddress.flatMap { try? HomeNodeURL(validating: $0) })
        }
    }
}
