import AgencyWidget
import Foundation

/// Where the widgets' file is written, and how the system is told to show it again. The app's is the app group's
/// container and WidgetKit.
public protocol WidgetFileSink {
    func write(_ snapshot: WidgetSnapshot) throws
    func clear()
    func reloadWidgets()
}

/// Writes what the widgets show after the desk is read, and removes it for good when the home node is left.
///
/// The widgets get only the file: the count of requests that are the viewer's and the first three, with their titles
/// unless the owner or the node keeps them off. The Access token and the device key stay in the app.
@MainActor
public final class WidgetPublisher {
    /// The file is written again after this long even when nothing changed, so the widget's "updated" mark stays honest.
    public static let rewriteAfter: TimeInterval = 10 * 60

    /// The owner's choice about titles and what the node says about them.
    public let preferences: WidgetPreferencesStore

    private let host: String
    private let sink: any WidgetFileSink
    private var last: WidgetSnapshot?
    private var items: [DeskItem]?
    private var isClosed = false

    public init(host: String, preferences: WidgetPreferencesStore, sink: any WidgetFileSink) {
        self.host = host
        self.preferences = preferences
        self.sink = sink
        preferences.onChange = { [weak self] in self?.republish() }
    }

    /// Writes the desk as the widgets show it. A write that would say the same as the last one is skipped, because the
    /// system budgets how often a widget may be reloaded.
    public func publish(items: [DeskItem], now: Date = Date()) {
        guard !isClosed else { return }
        self.items = items
        write(items, now: now)
    }

    /// Removes the file, for when the home node is left or the key is reset: nothing of it stays where a widget can
    /// show it. A read that was under way for that home node cannot write it again: the publisher stays closed, and it
    /// holds no desk to write when the choice about titles changes.
    public func clear() {
        isClosed = true
        items = nil
        last = nil
        sink.clear()
        sink.reloadWidgets()
    }

    private func republish() {
        guard let items else { return }
        write(items, now: Date())
    }

    private func write(_ items: [DeskItem], now: Date) {
        let snapshot = WidgetSnapshotBuilder.snapshot(host: host, items: items, now: now, showsTitles: preferences.showsTitles)
        if let last, last.hasSameContent(as: snapshot), now.timeIntervalSince(last.writtenAt) < Self.rewriteAfter { return }
        do {
            try sink.write(snapshot)
            last = snapshot
            sink.reloadWidgets()
        } catch {
            // The widget keeps showing the file it has, with its age; the next read writes again.
        }
    }
}
