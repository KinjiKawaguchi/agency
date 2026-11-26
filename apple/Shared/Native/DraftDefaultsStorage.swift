import AgencyModel
import Foundation

/// Drafts that were typed and not sent, kept in this app's defaults per home node.
struct DraftDefaultsStorage: DraftStorage {
    private let key: String

    private static func key(host: String) -> String { "agency.drafts.\(host)" }

    /// Removes what this device kept for the home node at `host`.
    static func forget(host: String) { UserDefaults.standard.removeObject(forKey: key(host: host)) }

    init(host: String) {
        key = Self.key(host: host)
    }

    func load() -> [String: DeskDraft] {
        UserDefaults.standard.data(forKey: key).flatMap { try? JSONDecoder().decode([String: DeskDraft].self, from: $0) } ?? [:]
    }

    func save(_ drafts: [String: DeskDraft]) {
        if drafts.isEmpty {
            UserDefaults.standard.removeObject(forKey: key)
        } else if let data = try? JSONEncoder().encode(drafts) {
            UserDefaults.standard.set(data, forKey: key)
        }
    }
}
