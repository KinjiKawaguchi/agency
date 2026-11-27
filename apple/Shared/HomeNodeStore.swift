import AgencyAPI
import AgencyKit
import AgencyModel
import Foundation
import Observation

/// The home nodes this device knows, kept across launches: the current one, the ones the owner added, and what the
/// current node says about the owner's other nodes (`HomeNodeBook`). Addresses and names only: the sign-ins and keys
/// are kept per host elsewhere and never move when the owner switches.
@MainActor
@Observable
final class HomeNodeStore {
    private static let bookKey = "homeNodeBook"
    /// What an earlier version kept: the one address.
    private static let legacyKey = "homeNodeURL"

    @ObservationIgnored private let defaults: UserDefaults
    /// The number on the icon, for the app that shows it; none for what only reads the list (the background refresh).
    @ObservationIgnored private let badges: BadgeKeeper?
    private(set) var book: HomeNodeBook

    init(defaults: UserDefaults = .standard, badges: BadgeKeeper? = nil) {
        self.defaults = defaults
        self.badges = badges
        book = HomeNodeBook(decoding: defaults.data(forKey: Self.bookKey), legacyAddress: defaults.string(forKey: Self.legacyKey))
        syncBadges()
    }

    var homeNode: HomeNodeURL? { book.current }
    var others: [HomeNodeEntry] { book.others }

    func save(_ node: HomeNodeURL) { change { $0.switchTo(node) } }
    func switchTo(_ node: HomeNodeURL) { change { $0.switchTo(node) } }
    func add(_ node: HomeNodeURL, label: String) { change { $0.add(node, label: label) } }
    func note(configured: [HomeNodeRef]) { change { $0.note(configured: configured) } }
    func note(nodeID: String?, of node: HomeNodeURL) { change { $0.note(nodeID: nodeID, of: node) } }
    func nodeID(of node: HomeNodeURL) -> String { book.nodeID(of: node) }
    func hasSaidID(of node: HomeNodeURL) -> Bool { book.hasSaidID(of: node) }

    /// Leaves the current node: the setup comes back. It is signed out, so its count no longer counts on the icon.
    func clear() {
        let leaving = book.current.map { book.nodeID(of: $0) }
        change { $0.leaveCurrent() }
        if let leaving { badges?.forget(leaving) }
    }

    /// The open node has `count` requests waiting for the owner: it is on the icon with the other nodes' counts.
    func recordWaiting(_ count: Int, for node: HomeNodeURL) {
        badges?.record(count, for: book.nodeID(of: node))
    }

    /// Takes a node the owner added off the list, and everything this device kept for its host when no other node on the
    /// device has that host (the sign-in, key, drafts and choices are kept by host alone). The current node is never forgotten.
    func forget(_ node: HomeNodeURL) {
        var next = book
        let mayRemoveSecrets = next.forget(node)
        keep(next)
        if mayRemoveSecrets { HomeNodeSecrets.forget(host: node.host) }
    }

    private func change(_ edit: (inout HomeNodeBook) -> Void) {
        var next = book
        edit(&next)
        keep(next)
    }

    private func keep(_ next: HomeNodeBook) {
        guard next != book else { return }
        book = next
        defaults.set(next.encoded(), forKey: Self.bookKey)
        defaults.removeObject(forKey: Self.legacyKey)
        syncBadges()
    }

    /// The nodes this device has: the open one and the ones on its list. A node that is not one any more loses its count.
    private func syncBadges() {
        badges?.activate(Set(book.activeNodes.map { book.nodeID(of: $0) }))
    }
}

/// What this device keeps for one home node, by its host: forgetting the node removes all of it.
enum HomeNodeSecrets {
    @MainActor
    static func forget(host: String) {
        AccessTokenStore(host: host).clear()
        do {
            try SecureEnclaveSigner.deleteKey(host: host)
        } catch {
            // The key stays in the Keychain for this host: forgetting the node goes on, and adding it again finds the same key.
            SecureEnclaveVault.logger.error("The device key could not be deleted when the node was forgotten: \("\(error)", privacy: .public)")
        }
        DraftDefaultsStorage.forget(host: host)
        FileOutboxStorage.forget(host: host)
        NotificationDefaultsStorage.forget(host: host)
        WidgetDefaultsStorage.forget(host: host)
    }
}
