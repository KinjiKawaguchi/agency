import AgencyAPI
import Foundation
import Observation

/// The participants and work screen: every team the account can read, each with its participants, work and epics.
@MainActor
@Observable
public final class PeopleStore {
    public enum Loading: Equatable, Sendable {
        case idle
        case loading
        case loaded
        case failed(String)
    }

    public private(set) var loading: Loading = .idle
    /// The Access session ended: the sign-in has to run again before anything else works.
    public private(set) var needsSignIn = false
    /// The teams that were read, in the order the node lists them.
    public private(set) var sections: [PeopleSection] = []
    /// Teams that could not be read, by `org/team`, with the reason.
    public private(set) var failures: [String: String] = [:]

    @ObservationIgnored private let source: any TeamSnapshotSource

    public init(source: any TeamSnapshotSource) {
        self.source = source
    }

    public func refresh() async {
        loading = .loading
        do {
            let me = try await source.me()
            let order = me.orgs.flatMap { org in org.teams.map { TeamReader.key(org: org.id, team: $0.id) } }
            var read: [String: Team] = [:]
            var failed: [String: TeamFailure] = [:]
            await TeamReader.read(me.orgs, from: source) { key, result in
                switch result {
                case .success(let snapshot): read[key] = Team(snapshot: snapshot)
                case .failure(let error): failed[key] = TeamFailure(error: error)
                }
            }
            sections = order.compactMap { read[$0].map(PeopleSection.init(team:)) }
            failures = failed.mapValues(\.reason)
            // A session that ended while the teams were being read shows as that, not as failures of each team.
            needsSignIn = failed.values.contains { $0.signInEnded }
            loading = .loaded
        } catch let error as NodeError where error == .sessionExpired || error == .notSignedIn {
            needsSignIn = true
            loading = .failed(Ja.Desk.signInEnded)
        } catch {
            loading = .failed(DeskStore.reason(error))
        }
    }
}
