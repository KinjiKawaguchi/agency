import AgencyAPI
import Foundation
import Observation

/// Where the overview reads from: the home node's Client API. `NodeClient` is the real one.
public protocol OverviewSource: Sendable {
    func me() async throws -> Me
    func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot
    func usage(org: String, team: String, days: Int) async throws -> TokenUsage
}

extension NodeClient: OverviewSource {}

/// What is known of a team's cost over the last week.
public enum WeekCost: Equatable, Sendable {
    case loading
    case known(Double)
    /// The team's runtime does not report usage.
    case none
    case failed
}

/// Every org and team of the account on one screen: what needs the viewer first, and what funds the work.
@MainActor
@Observable
public final class OverviewStore {
    public enum Loading: Equatable, Sendable {
        case idle
        case loading
        case loaded
        case failed(String)
    }

    /// The days the cost on a team's card covers.
    public static let usageDays = 7

    public private(set) var loading: Loading = .idle
    /// The Access session ended: the sign-in has to run again before anything else works.
    public private(set) var needsSignIn = false
    public private(set) var orgs: [OrgRef] = []
    public private(set) var funding: [Funding] = []
    private(set) var states: [String: TeamState] = [:]
    public private(set) var weekCosts: [String: WeekCost] = [:]

    @ObservationIgnored private let source: any OverviewSource

    public init(source: any OverviewSource) {
        self.source = source
    }

    // MARK: What the screen shows

    /// An org with its teams in the order they are shown.
    public struct OrgSection: Equatable, Sendable {
        public let org: OrgRef
        public let teams: [TeamEntry]

        public init(org: OrgRef, teams: [TeamEntry]) {
            self.org = org
            self.teams = teams
        }
    }

    /// Orgs where their most pressing team is, then by name; teams likewise.
    public var sections: [OrgSection] {
        orgs
            .map { org in
                let entries = org.teams.map { TeamEntry(org: org.id, ref: $0, state: states["\(org.id)/\($0.id)"] ?? .loading) }
                return OrgSection(org: org, teams: OverviewOrder.teams(entries))
            }
            .sorted { left, right in
                let (leftRank, rightRank) = (OverviewOrder.rank(ofOrgWith: left.teams), OverviewOrder.rank(ofOrgWith: right.teams))
                if leftRank != rightRank { return leftRank < rightRank }
                return OverviewOrder.byName(left.org.label, right.org.label) ?? (left.org.id < right.org.id)
            }
    }

    /// Requests waiting on the viewer in every team read so far, and how many teams they are in.
    public var waiting: (requests: Int, teams: Int) {
        let counts = states.values.compactMap { state -> Int? in
            guard case .ready(let team) = state else { return nil }
            return team.mine.count
        }
        return (counts.reduce(0, +), counts.filter { $0 > 0 }.count)
    }

    public func weekCost(of entry: TeamEntry) -> WeekCost { weekCosts[entry.key] ?? .loading }

    public func fundingSummaries(now: Date = Date()) -> [FundingSummary] {
        funding.map { FundingSummary(funding: $0, now: now) }
    }

    // MARK: Reading

    /// Reads the account, then every team and its week's cost. A team that cannot be read is shown as that, in its place.
    public func refresh() async {
        loading = .loading
        do {
            let me = try await source.me()
            orgs = me.orgs
            funding = me.funding
            needsSignIn = false
            let keys = Set(me.orgs.flatMap { org in org.teams.map { "\(org.id)/\($0.id)" } })
            states = states.filter { keys.contains($0.key) }
            weekCosts = weekCosts.filter { keys.contains($0.key) }
            await readTeams(of: me.orgs)
            loading = .loaded
        } catch NodeError.sessionExpired {
            needsSignIn = true
            loading = .failed(Ja.Desk.signInEnded)
        } catch NodeError.notSignedIn {
            needsSignIn = true
            loading = .failed(Ja.Desk.signInEnded)
        } catch {
            loading = .failed(DeskStore.reason(error))
        }
    }

    private func readTeams(of orgs: [OrgRef]) async {
        let source = source
        await withTaskGroup(of: (String, Result<TeamSnapshot, Error>).self) { group in
            for org in orgs {
                for team in org.teams {
                    group.addTask {
                        do { return ("\(org.id)/\(team.id)", .success(try await source.teamSnapshot(org: org.id, team: team.id))) }
                        catch { return ("\(org.id)/\(team.id)", .failure(error)) }
                    }
                }
            }
            for await (key, result) in group { apply(result, to: key) }
        }
        await readCosts()
    }

    private func apply(_ result: Result<TeamSnapshot, Error>, to key: String) {
        switch result {
        case .success(let snapshot):
            states[key] = .ready(Team(snapshot: snapshot))
        case .failure(let error):
            // A team that was read keeps its last answer, as the web client does, until it can be read again.
            if case .ready = states[key] { return }
            let isEnded = (error as? NodeError).map { $0 == .sessionExpired || $0 == .notSignedIn } ?? false
            states[key] = .failed(reason: DeskStore.reason(error), signInEnded: isEnded)
        }
    }

    /// The week's cost of every team whose node can answer: a copy that could not be renewed says its node is out of reach.
    private func readCosts() async {
        let source = source
        let asked = orgs.flatMap { org in
            org.teams.compactMap { team -> (key: String, org: String, team: String)? in
                let key = "\(org.id)/\(team.id)"
                guard case .ready(let snapshot)? = states[key], !snapshot.isUnreachable else { return nil }
                return (key, org.id, team.id)
            }
        }
        for entry in asked where weekCosts[entry.key] == nil { weekCosts[entry.key] = .loading }
        await withTaskGroup(of: (String, WeekCost).self) { group in
            for entry in asked {
                group.addTask {
                    do {
                        let usage = try await source.usage(org: entry.org, team: entry.team, days: Self.usageDays)
                        return (entry.key, .known(usage.byPerson.reduce(0) { $0 + $1.costUsd }))
                    } catch NodeError.http(404, _) {
                        return (entry.key, WeekCost.none)
                    } catch {
                        return (entry.key, WeekCost.failed)
                    }
                }
            }
            for await (key, cost) in group { weekCosts[key] = cost }
        }
    }
}
