import AgencyKit
import Foundation
import UserNotifications

/// Keeps the number on the icon: the sum, over the home nodes this device has, of each node's last count (`BadgeLedger`).
///
/// The ledger is in the app group's container, where the notification service extension also writes the count of a node whose
/// alert arrives while the app is closed. Where there is no group (a build without one, and the Mac, which has no extension),
/// the number is the open node's own count, as it was for one node.
@MainActor
final class BadgeKeeper {
    private let file: BadgeLedgerFile?
    private let show: (Int) -> Void

    init(file: BadgeLedgerFile? = BadgeKeeper.groupFile, show: @escaping (Int) -> Void = BadgeKeeper.showOnIcon) {
        self.file = file
        self.show = show
    }

    /// The nodes this device has, by id: a node that is not one any more loses its count.
    func activate(_ nodes: Set<String>) {
        guard let file else { return }
        show(file.update { $0.activating(nodes) }.total)
    }

    /// The node has `count` waiting, as its desk says.
    func record(_ count: Int, for node: String) {
        guard let file else { return show(count) }
        show(file.update { $0.recording(count, for: node) }.total)
    }

    /// The node is signed out: its count no longer counts, though it may stay on the list.
    func forget(_ node: String) {
        guard let file else { return show(0) }
        show(file.update { $0.forgetting(node) }.total)
    }

    private static var groupFile: BadgeLedgerFile? {
        (Bundle.main.object(forInfoDictionaryKey: "AgencyAppGroup") as? String).flatMap(BadgeLedgerFile.appGroup)
    }

    /// Without the badge permission the system ignores it.
    private nonisolated static func showOnIcon(_ count: Int) {
        Task { try? await UNUserNotificationCenter.current().setBadgeCount(count) }
    }
}
