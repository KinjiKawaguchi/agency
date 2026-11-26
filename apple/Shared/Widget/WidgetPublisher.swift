import AgencyModel
import AgencyWidget
import Foundation
import WidgetKit

/// Writes what the widgets show into the app group, after the desk is read, and asks the system to reload them.
///
/// The widgets get only the file: the count of requests that are the viewer's and the first three, with their titles
/// unless the owner or the node keeps them off. The Access token and the device key stay in the app.
@MainActor
final class WidgetPublisher {
    /// The file is written again after this long even when nothing changed, so the widget's "updated" mark stays honest.
    private static let rewriteAfter: TimeInterval = 10 * 60

    /// The owner's choice about titles and what the node says about them.
    let preferences: WidgetPreferencesStore

    private let host: String
    private let store: WidgetSnapshotStore?
    private var last: WidgetSnapshot?
    private var items: [DeskItem]?

    init(host: String, preferences: WidgetPreferencesStore) {
        self.host = host
        self.preferences = preferences
        store = (Bundle.main.object(forInfoDictionaryKey: "AgencyAppGroup") as? String).flatMap(WidgetSnapshotStore.appGroup)
        preferences.onChange = { [weak self] in self?.republish() }
    }

    /// Writes the desk as the widgets show it. A write that would say the same as the last one is skipped, because the
    /// system budgets how often a widget may be reloaded.
    func publish(items: [DeskItem], now: Date = Date()) {
        self.items = items
        write(items, now: now)
    }

    /// Removes the file, for when the home node is left or the key is reset: nothing of it stays where a widget can
    /// show it, and nothing is written again until the desk is read again.
    func clear() {
        items = nil
        last = nil
        store?.clear()
        WidgetCenter.shared.reloadAllTimelines()
    }

    private func republish() {
        guard let items else { return }
        write(items, now: Date())
    }

    private func write(_ items: [DeskItem], now: Date) {
        guard let store else { return }
        let snapshot = WidgetSnapshotBuilder.snapshot(host: host, items: items, now: now, showsTitles: preferences.showsTitles)
        if let last, last.hasSameContent(as: snapshot), now.timeIntervalSince(last.writtenAt) < Self.rewriteAfter { return }
        do {
            try store.write(snapshot)
            last = snapshot
            WidgetCenter.shared.reloadAllTimelines()
        } catch {
            // The widget keeps showing the file it has, with its age; the next read writes again.
        }
    }
}
