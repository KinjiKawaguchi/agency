import AgencyModel
import Foundation

/// Drafts that were typed and not sent, kept in this app's defaults per home node.
struct DraftDefaultsStorage: DraftStorage {
    private let key: String

    init(host: String) {
        key = "agency.drafts.\(host)"
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
