import AgencyModel
import AgencyWidget
import Foundation
import WidgetKit

/// Writes what the widgets show into the app group, after the desk is read, and asks the system to reload them.
///
/// The widgets get only the file: the count of requests that are the viewer's and the first three. The Access token
/// and the device key stay in the app.
@MainActor
final class WidgetPublisher {
    /// The file is written again after this long even when nothing changed, so the widget's "updated" mark stays honest.
    private static let rewriteAfter: TimeInterval = 10 * 60

    private let host: String
    private let store: WidgetSnapshotStore?
    private var last: WidgetSnapshot?

    init(host: String) {
        self.host = host
        store = (Bundle.main.object(forInfoDictionaryKey: "AgencyAppGroup") as? String).flatMap(WidgetSnapshotStore.appGroup)
    }

    /// Writes the desk as the widgets show it. A write that would say the same as the last one is skipped, because the
    /// system budgets how often a widget may be reloaded.
    func publish(items: [DeskItem], now: Date = Date()) {
        guard let store else { return }
        let snapshot = WidgetSnapshotBuilder.snapshot(host: host, items: items, now: now)
        if let last, last.hasSameContent(as: snapshot), now.timeIntervalSince(last.writtenAt) < Self.rewriteAfter { return }
        do {
            try store.write(snapshot)
            last = snapshot
            WidgetCenter.shared.reloadAllTimelines()
        } catch {
            // The widget keeps showing the file it has, with its age; the next read writes again.
        }
    }

    /// Removes the file, for when the home node is left: nothing of it stays where a widget can show it.
    func clear() {
        store?.clear()
        last = nil
        WidgetCenter.shared.reloadAllTimelines()
    }
}
