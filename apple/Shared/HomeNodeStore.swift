import AgencyKit
import Foundation
import Observation

/// The home node address the owner entered, kept across launches.
@MainActor
@Observable
final class HomeNodeStore {
    private static let key = "homeNodeURL"

    @ObservationIgnored private let defaults: UserDefaults
    private(set) var homeNode: HomeNodeURL?

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        homeNode = defaults.string(forKey: Self.key).flatMap { try? HomeNodeURL(validating: $0) }
    }

    func save(_ node: HomeNodeURL) {
        defaults.set(node.url.absoluteString, forKey: Self.key)
        homeNode = node
    }

    func clear() {
        defaults.removeObject(forKey: Self.key)
        homeNode = nil
    }
}
