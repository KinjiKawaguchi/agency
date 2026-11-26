import AgencyModel
import AgencyWidget
import Foundation
import WidgetKit

/// The widgets' file in the app group's container, and WidgetKit.
struct WidgetAppGroupSink: WidgetFileSink {
    private let store: WidgetSnapshotStore?

    init() {
        store = (Bundle.main.object(forInfoDictionaryKey: "AgencyAppGroup") as? String).flatMap(WidgetSnapshotStore.appGroup)
    }

    func write(_ snapshot: WidgetSnapshot) throws {
        try store?.write(snapshot)
    }

    func clear() {
        store?.clear()
    }

    func reloadWidgets() {
        WidgetCenter.shared.reloadAllTimelines()
    }
}
