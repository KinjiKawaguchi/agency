import AgencyAPI
import Foundation
import Observation

/// Where the outcomes page reads from: the home node's Client API. `NodeClient` is the real one.
public protocol OutcomesSource: Sendable {
    func me() async throws -> Me
    func outcomes(org: String, days: Int) async throws -> Outcomes
    func readings(org: String) async throws -> Readings
}

extension NodeClient: OutcomesSource {}

/// What is known of the readings of one org's aims.
public enum OrgReadings: Equatable, Sendable {
    /// One line for each aim of each team's plan; none when no team has an aim.
    case lines([ReadingLine])
    /// The node does not serve the readings (404).
    case unavailable
    case failed(String)
}

/// One org's numbers as the page draws them.
public struct OrgOutcomes: Equatable, Identifiable, Sendable {
    public let id: String
    public let label: String
    public let cards: [KpiCard]
    public let readings: OrgReadings
}

/// The owner's numbers for what each org measures, over a period the person chooses.
///
/// Only the owner may read them: a node answers anyone else 403, and the store then has nothing to show, so the page is
/// not offered (`isAvailable`) and nothing says why. They are business data: kept in memory only, never written to the
/// disk, a widget, a notification or a log, and gone with the session.
@MainActor
@Observable
public final class OutcomesStore {
    public enum Loading: Equatable, Sendable {
        case idle
        case loading
        case loaded
        case failed(String)
    }

    /// The periods to choose from, in days. A number is kept monthly more often than not, so the longest is the default.
    public static let periods = [30, 90]
    public static let defaultDays = 90

    public private(set) var loading: Loading = .idle
    /// The Access session ended: the sign-in has to run again before anything else works.
    public private(set) var needsSignIn = false
    public private(set) var days = OutcomesStore.defaultDays
    /// The orgs the owner may read, in the account's order.
    public private(set) var orgs: [OrgOutcomes] = []
    /// Whether a read has finished at least once. Set once and never reset, like the desk's: a refresh under way or a
    /// failed one does not make the page unread again.
    public private(set) var hasAnswered = false
    /// Whether the node has let the owner read the numbers of at least one org. The page is offered only then: not while
    /// it loads, not on a refusal, not when the read failed before it ever worked. Once a read worked it stays offered
    /// until a read finishes with every org refusing.
    public private(set) var isAvailable = false

    @ObservationIgnored private let source: any OutcomesSource
    @ObservationIgnored private let retry: RetryPolicy

    public init(source: any OutcomesSource, retry: RetryPolicy = .standard) {
        self.source = source
        self.retry = retry
    }

    // MARK: Reading

    public func refresh() async {
        let before = loading
        loading = .loading
        do {
            let me = try await source.me()
            needsSignIn = false
            let (days, source, retry) = (days, source, retry)
            let results = await withTaskGroup(of: (Int, OrgRead).self) { group in
                for (index, org) in me.orgs.enumerated() {
                    group.addTask { (index, await Self.read(org, days: days, source: source, retry: retry)) }
                }
                var found: [(Int, OrgRead)] = []
                for await result in group { found.append(result) }
                return found.sorted { $0.0 < $1.0 }.map(\.1)
            }
            // A period chosen meanwhile has its own answer coming; this one is for the old one.
            guard days == self.days else { return }
            apply(results, before: before)
        } catch let error as NodeError where error == .sessionExpired || error == .notSignedIn {
            needsSignIn = true
            loading = .failed(Ja.Desk.signInEnded)
        } catch is CancellationError {
            // A read that was cancelled says nothing: what was there stays.
            loading = before
        } catch {
            loading = .failed(DeskStore.reason(error))
        }
    }

    public func choose(days: Int) async {
        guard Self.periods.contains(days), days != self.days else { return }
        self.days = days
        await refresh()
    }

    /// What one org's read came to.
    private enum OrgRead: Sendable {
        case answer(OrgOutcomes)
        /// The owner may not read it (403), or the org is not one the node can answer (404): nothing to show, and no word of it.
        case refused
        case failed(String)
        /// The Access session ended while the org was read.
        case signInEnded
        case cancelled
    }

    private static func read(_ org: OrgRef, days: Int, source: any OutcomesSource, retry: RetryPolicy) async -> OrgRead {
        let id = org.id
        let outcomes: Outcomes
        do {
            outcomes = try await retry.run { try await source.outcomes(org: id, days: days) }
        } catch NodeError.http(403, _), NodeError.http(404, _) {
            return .refused
        } catch is CancellationError {
            return .cancelled
        } catch let error as NodeError where error == .sessionExpired || error == .notSignedIn {
            return .signInEnded
        } catch {
            return .failed(DeskStore.reason(error))
        }
        let readings = await readReadings(org, kpis: outcomes.series.map(\.kpi), source: source, retry: retry)
        return .answer(OrgOutcomes(id: id, label: org.label, cards: outcomes.series.map(KpiCard.init(series:)), readings: readings))
    }

    private static func readReadings(_ org: OrgRef, kpis: [Kpi], source: any OutcomesSource, retry: RetryPolicy) async -> OrgReadings {
        let id = org.id
        do {
            let answer = try await retry.run { try await source.readings(org: id) }
            let kpiOf = Dictionary(kpis.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
            let teamLabel = Dictionary(org.teams.map { ($0.id, $0.label) }, uniquingKeysWith: { first, _ in first })
            return .lines(answer.readings.map { ReadingLine(reading: $0, team: teamLabel[$0.team] ?? $0.team, kpi: kpiOf[$0.kpi]) })
        } catch NodeError.http(404, _), NodeError.http(403, _) {
            return .unavailable
        } catch {
            return .failed(DeskStore.reason(error))
        }
    }

    private func apply(_ results: [OrgRead], before: Loading) {
        var answers: [OrgOutcomes] = []
        var failure: String?
        var isCancelled = false
        var isSignInEnded = false
        for result in results {
            switch result {
            case .answer(let outcomes): answers.append(outcomes)
            case .refused: break
            case .failed(let reason): failure = failure ?? reason
            case .signInEnded: isSignInEnded = true
            case .cancelled: isCancelled = true
            }
        }
        if isSignInEnded {
            needsSignIn = true
            loading = .failed(Ja.Desk.signInEnded)
            return
        }
        if isCancelled {
            loading = before
            return
        }
        // A read that could not finish for some org says nothing of whether the owner may read, so what is shown stays; the
        // failure is said, over what was read.
        if answers.isEmpty, let failure {
            loading = .failed(failure)
            return
        }
        orgs = answers
        isAvailable = !answers.isEmpty
        hasAnswered = true
        loading = failure.map { .failed($0) } ?? .loaded
    }
}
