import AgencyModel
import Foundation

/// What the owner chose about the widgets, kept in this app's defaults per home node.
struct WidgetDefaultsStorage: WidgetPreferencesStorage {
    private let key: String

    init(host: String) {
        key = "agency.widgets.\(host)"
    }

    func load() -> WidgetPreferences {
        guard UserDefaults.standard.object(forKey: key) != nil else { return .standard }
        return WidgetPreferences(showsTitles: UserDefaults.standard.bool(forKey: key))
    }

    func save(_ preferences: WidgetPreferences) {
        UserDefaults.standard.set(preferences.showsTitles, forKey: key)
    }
}
