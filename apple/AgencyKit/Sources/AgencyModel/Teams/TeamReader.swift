import AgencyAPI
import Foundation

/// What a screen that lists teams reads: the account and each team's view.
public protocol TeamSnapshotSource: Sendable {
    func me() async throws -> Me
    func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot
}

extension NodeClient: TeamSnapshotSource {}

/// Where a team's token use is read from.
public protocol TeamUsageSource: Sendable {
    func usage(org: String, team: String, days: Int) async throws -> TokenUsage
}

extension NodeClient: TeamUsageSource {}

/// What the overview and the costs screen read from: the home node's Client API. `NodeClient` is the real one.
public protocol OverviewSource: TeamSnapshotSource, TeamUsageSource {}

extension NodeClient: OverviewSource {}

/// Why a team could not be read.
public struct TeamFailure: Equatable, Sendable {
    public let reason: String
    /// The Access session is over: the sign-in has to run again before anything can be read.
    public let signInEnded: Bool

    @MainActor
    public init(error: Error) {
        reason = DeskStore.reason(error)
        signInEnded = (error as? NodeError).map { $0 == .sessionExpired || $0 == .notSignedIn } ?? false
    }
}

/// Reads every team of the given orgs at once, handing each outcome over as it arrives, so one slow
/// team does not hold back the others.
enum TeamReader {
    /// `org/team`, the key a team is known by across orgs.
    static func key(org: String, team: String) -> String { "\(org)/\(team)" }

    @MainActor
    static func read(
        _ orgs: [OrgRef], from source: some TeamSnapshotSource,
        each: @MainActor (String, Result<TeamSnapshot, Error>) -> Void
    ) async {
        await withTaskGroup(of: (String, Result<TeamSnapshot, Error>).self) { group in
            for org in orgs {
                for team in org.teams {
                    let key = key(org: org.id, team: team.id)
                    group.addTask {
                        do { return (key, .success(try await source.teamSnapshot(org: org.id, team: team.id))) }
                        catch { return (key, .failure(error)) }
                    }
                }
            }
            for await (key, result) in group { each(key, result) }
        }
    }
}
