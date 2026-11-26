import AgencyModel
import Foundation

/// What the owner wants to be told, kept in this app's defaults per home node.
struct NotificationDefaultsStorage: NotificationPreferencesStorage {
    private let key: String

    private static func key(host: String) -> String { "agency.notifications.\(host)" }

    /// Removes what this device kept for the home node at `host`.
    static func forget(host: String) { UserDefaults.standard.removeObject(forKey: key(host: host)) }

    init(host: String) {
        key = Self.key(host: host)
    }

    func load() -> NotificationPreferences? {
        UserDefaults.standard.data(forKey: key).flatMap { try? JSONDecoder().decode(NotificationPreferences.self, from: $0) }
    }

    func save(_ preferences: NotificationPreferences) {
        if let data = try? JSONEncoder().encode(preferences) {
            UserDefaults.standard.set(data, forKey: key)
        }
    }
}
