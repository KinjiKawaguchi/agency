import AgencyModel
import Foundation

/// What the owner chose about the widgets, kept in this app's defaults per home node.
struct WidgetDefaultsStorage: WidgetPreferencesStorage {
    private let key: String

    private static func key(host: String) -> String { "agency.widgets.\(host)" }

    /// Removes what this device kept for the home node at `host`.
    static func forget(host: String) { UserDefaults.standard.removeObject(forKey: key(host: host)) }

    init(host: String) {
        key = Self.key(host: host)
    }

    func load() -> WidgetPreferences {
        guard UserDefaults.standard.object(forKey: key) != nil else { return .standard }
        return WidgetPreferences(showsTitles: UserDefaults.standard.bool(forKey: key))
    }

    func save(_ preferences: WidgetPreferences) {
        UserDefaults.standard.set(preferences.showsTitles, forKey: key)
    }
}
