import AgencyAPI
import Foundation
import Observation

/// What is known of one team's token use.
public enum UsageState: Equatable, Sendable {
    case loading
    case ready(TokenUsage)
    /// The team's runtime does not report usage (404).
    case unavailable
    /// The team's node or runtime could not answer just now (503), with its words.
    case couldNotAnswer(String)
    case failed(String)
}

extension UsageState {
    /// The read kept failing after it was asked again (or failed without an answer), so what was shown before is still the best there is.
    var isRetryExhausted: Bool {
        switch self {
        case .couldNotAnswer, .failed: true
        case .loading, .ready, .unavailable: false
        }
    }
}

/// One team on the costs screen: who is in it, and what it used.
public struct CostsTeam: Equatable, Sendable, Identifiable {
    public let team: Team
    public let usage: UsageState
    public var id: String { team.key }

    public init(team: Team, usage: UsageState) {
        self.team = team
        self.usage = usage
    }

    /// The report to draw, when the usage was read.
    public var report: TokenReport? {
        if case .ready(let usage) = usage { TokenReport(usage: usage, view: team.view) } else { nil }
    }
}

/// The costs screen: the funding sources with their windows, and each team's token use over a period.
@MainActor
@Observable
public final class CostsStore {
    public enum Loading: Equatable, Sendable {
        case idle
        case loading
        case loaded
        case failed(String)
    }

    /// The periods to choose from, in days.
    public static let periods = [7, 14, 30]

    public private(set) var loading: Loading = .idle
    /// Whether a read has finished at least once. Set once and never reset: a refresh under way, a failed one or a cancelled one
    /// does not make the screen unread again, so an account with nothing in it keeps saying so instead of going back to bars.
    public private(set) var hasAnswered = false
    /// The Access session ended: the sign-in has to run again before anything else works.
    public private(set) var needsSignIn = false
    public private(set) var days = 14
    public private(set) var funding: [Funding] = []
    public private(set) var teams: [CostsTeam] = []

    @ObservationIgnored private let source: any OverviewSource
    @ObservationIgnored private let retry: RetryPolicy
    @ObservationIgnored private let flight = SingleFlight()
    @ObservationIgnored private var refs: [(key: String, org: String, team: String)] = []
    @ObservationIgnored private var lastRead: [String: Team] = [:]
    @ObservationIgnored private var usage: [String: UsageState] = [:]

    public init(source: any OverviewSource, retry: RetryPolicy = .standard) {
        self.source = source
        self.retry = retry
    }

    public func fundingSummaries(now: Date = Date()) -> [FundingSummary] {
        funding.map { FundingSummary(funding: $0, now: now) }
    }

    // MARK: Reading

    /// Reads again. A read already running is waited for, not repeated.
    public func refresh() async {
        await flight.run { await self.reload() }
    }

    private func reload() async {
        let before = loading
        loading = .loading
        do {
            let me = try await source.me()
            funding = me.funding
            refs = me.orgs.flatMap { org in org.teams.map { (TeamReader.key(org: org.id, team: $0.id), org.id, $0.id) } }
            var read = lastRead.filter { key, _ in refs.contains { $0.key == key } }
            var endedMidway = false
            await TeamReader.read(me.orgs, from: source, retry: retry) { key, result in
                switch result {
                case .success(let snapshot): read[key] = Team(snapshot: snapshot)
                case .failure(let error): endedMidway = endedMidway || TeamFailure(error: error).signInEnded
                }
            }
            lastRead = read
            needsSignIn = endedMidway
            usage = usage.filter { key, _ in read[key] != nil }
            publish()
            await readUsage()
            hasAnswered = true
            loading = .loaded
        } catch let error as NodeError where error == .sessionExpired || error == .notSignedIn {
            needsSignIn = true
            loading = .failed(Ja.Desk.signInEnded)
        } catch where DeskStore.isCancelled(error) {
            // A read that was cancelled says nothing: what was there stays.
            loading = before
        } catch {
            loading = .failed(DeskStore.reason(error))
        }
    }

    /// Reads every team's usage again for another period.
    public func choose(days: Int) async {
        guard Self.periods.contains(days), days != self.days else { return }
        self.days = days
        await readUsage()
    }

    private func readUsage() async {
        let source = source
        let retry = retry
        let days = days
        let asked = refs.filter { lastRead[$0.key] != nil }
        for entry in asked where usage[entry.key] == nil { usage[entry.key] = .loading }
        publish()
        await withTaskGroup(of: (String, UsageState).self) { group in
            for entry in asked {
                group.addTask {
                    do {
                        return (entry.key, .ready(try await retry.run { try await source.usage(org: entry.org, team: entry.team, days: days) }))
                    } catch NodeError.http(404, _) {
                        return (entry.key, .unavailable)
                    } catch NodeError.http(503, let message) {
                        return (entry.key, .couldNotAnswer(message ?? "HTTP 503"))
                    } catch {
                        return (entry.key, .failed(await DeskStore.reason(error)))
                    }
                }
            }
            for await (key, state) in group {
                // A period chosen meanwhile has its own answer coming; this one is for the old period.
                guard days == self.days else { continue }
                // Data on screen stays while a refresh fails; only a final answer (404) replaces it.
                if case .ready = usage[key], state.isRetryExhausted { continue }
                usage[key] = state
                publish()
            }
        }
    }

    private func publish() {
        teams = refs.compactMap { entry in
            lastRead[entry.key].map { CostsTeam(team: $0, usage: usage[entry.key] ?? .loading) }
        }
    }
}
