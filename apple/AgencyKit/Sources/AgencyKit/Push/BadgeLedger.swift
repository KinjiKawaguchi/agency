import Foundation

/// The number on the icon when the device has several home nodes: the sum of what each node last said it has waiting.
///
/// A push's `badge` is its node's own count, so the icon of a closed app would show the last node's count. The app and its
/// notification service extension keep each node's last count here, by the node's id (`agency.node`), and show the sum over the
/// nodes this device has (`active`). A count counts until the node says another, or until the node is forgotten or left.
/// A node's count changes only when its own push arrives or when the app reads its desk; a request answered elsewhere leaves it
/// where it was until then.
public struct BadgeLedger: Equatable, Codable, Sendable {
    /// Each active node's last count, by its id.
    public private(set) var counts: [String: Int]
    /// The nodes this device has (the open one and the ones on its list), by id. A push from another id adds nothing.
    public private(set) var active: Set<String>

    public init(counts: [String: Int] = [:], active: Set<String> = []) {
        self.active = active
        self.counts = counts.filter { active.contains($0.key) && $0.value >= 0 }
    }

    /// The number for the icon.
    public var total: Int { counts.values.reduce(0, +) }

    /// `node` has `count` waiting. A node that is not one of this device's, or a count below 0, changes nothing.
    public func recording(_ count: Int, for node: String) -> BadgeLedger {
        guard active.contains(node), count >= 0 else { return self }
        var next = self
        next.counts[node] = count
        return next
    }

    /// The nodes this device has: a node that is no longer one loses its count.
    public func activating(_ nodes: Set<String>) -> BadgeLedger {
        BadgeLedger(counts: counts, active: nodes)
    }

    /// `node` is signed out or forgotten: its count no longer counts, though it may stay on the list.
    public func forgetting(_ node: String) -> BadgeLedger {
        var next = self
        next.counts[node] = nil
        return next
    }
}

/// The ledger in a file of the app group's container, shared by the app and its notification service extension.
///
/// The two processes may write at nearly the same moment and the later write wins; a count lost that way is told again by the
/// node's next push or the next read of its desk. A file that cannot be read is an empty ledger.
public struct BadgeLedgerFile: Sendable {
    public static let fileName = "badge-ledger.json"
    private let url: URL

    public init(directory: URL) {
        url = directory.appendingPathComponent(Self.fileName)
    }

    /// The group container's file; `nil` where there is no group (or not on a phone).
    public static func appGroup(_ identifier: String) -> BadgeLedgerFile? {
        #if canImport(Darwin)
        FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: identifier).map(BadgeLedgerFile.init)
        #else
        nil
        #endif
    }

    public func load() -> BadgeLedger {
        guard let data = try? Data(contentsOf: url), let ledger = try? JSONDecoder().decode(BadgeLedger.self, from: data) else {
            return BadgeLedger()
        }
        return ledger
    }

    /// Reads the ledger, applies `change`, writes it, and returns it. A write that fails still returns the changed ledger, so the
    /// badge shows it.
    @discardableResult
    public func update(_ change: (BadgeLedger) -> BadgeLedger) -> BadgeLedger {
        let next = change(load())
        if let data = try? JSONEncoder().encode(next) { try? data.write(to: url, options: .atomic) }
        return next
    }
}

/// What the notification service extension does with a push, kept out of the extension so that it is tested.
public enum BadgeFromPush {
    /// The number the icon should show for an alert of a node, or `nil` to leave the notification as it came: when it names no
    /// node (a node that does not know its origin, or an older one), has no count, or names a node this device does not have.
    public static func badge(userInfo: [AnyHashable: Any], pushBadge: Int?, file: BadgeLedgerFile) -> Int? {
        guard let pushBadge, let node = NotificationTap.nodeID(in: userInfo) else { return nil }
        let ledger = file.load()
        guard ledger.active.contains(node) else { return nil }
        return file.update { $0.recording(pushBadge, for: node) }.total
    }
}
