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
    private(set) var book: HomeNodeBook

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        book = HomeNodeBook(decoding: defaults.data(forKey: Self.bookKey), legacyAddress: defaults.string(forKey: Self.legacyKey))
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

    /// Leaves the current node: the setup comes back.
    func clear() { change { $0.leaveCurrent() } }

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
    }
}

/// What this device keeps for one home node, by its host: forgetting the node removes all of it.
enum HomeNodeSecrets {
    @MainActor
    static func forget(host: String) {
        AccessTokenStore(host: host).clear()
        SecureEnclaveSigner.deleteKey(host: host)
        DraftDefaultsStorage.forget(host: host)
        NotificationDefaultsStorage.forget(host: host)
        WidgetDefaultsStorage.forget(host: host)
    }
}
