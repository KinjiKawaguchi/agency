import AgencyKit
import Foundation

/// The node's Access team host, kept per home node so that the web view knows it at the next launch
/// before the node has been asked again.
struct AccessTeamStore: Sendable {
    private let key: String

    init(host: String) {
        key = "agency.accessTeam.\(host)"
    }

    var host: String? {
        UserDefaults.standard.string(forKey: key).flatMap { AccessTeam.isTeamHost($0) ? $0 : nil }
    }

    func save(_ team: String) {
        guard AccessTeam.isTeamHost(team) else { return }
        UserDefaults.standard.set(team, forKey: key)
    }
}
