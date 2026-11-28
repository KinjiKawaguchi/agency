import AgencyAPI
import Foundation
import Observation

/// Where the evaluation reads from: the home node's Client API. `NodeClient` is the real one.
public protocol EvaluationSource: Sendable {
    func me() async throws -> Me
    func evaluation(org: String, days: Int) async throws -> Evaluation
    func effort(org: String, days: Int) async throws -> Effort
    /// What the org intends for each of its teams, from its definition.
    func orgPlans(org: String) async throws -> OrgPlans
    /// What the node says of itself, for the capabilities it lists.
    func about() async throws -> About
    /// Where a team's week stands against its plan's weekly budget, as the node works it out; only a node that lists `budget` is asked.
    func budget(org: String, team: String) async throws -> BudgetStatus
}

extension NodeClient: EvaluationSource {}

/// What is known of an org's evaluation.
public enum EvaluationState: Equatable, Sendable {
    case loading
    case ready([TeamEvaluationReport], unavailable: [TeamUnavailableNote])
    /// The node does not serve the route yet (404).
    case unavailable
    /// The node or a team's runtime could not answer, and kept not answering after it was asked again.
    case couldNotAnswer(String)
    case failed(String)
}

/// What is known of an org's effort: where each team's tokens went, by work type.
public enum EffortState: Equatable, Sendable {
    case loading
    case ready([TeamEffortReport], unavailable: [TeamUnavailableNote])
    /// The node does not serve the route yet (404).
    case unavailable
    case couldNotAnswer(String)
    case failed(String)

    var isRetryExhausted: Bool {
        switch self {
        case .couldNotAnswer, .failed: true
        case .loading, .ready, .unavailable: false
        }
    }
}

/// A team the node could not get numbers for, by the team's own label.
public struct TeamUnavailableNote: Equatable, Sendable, Identifiable {
    public let team: String
    public let label: String
    public let reason: String
    public var id: String { team }

    public init(team: String, label: String, reason: String) {
        self.team = team
        self.label = label
        self.reason = reason
    }
    public var title: String { Ja.Evaluation.team(label) }
    public var text: String { Ja.Evaluation.teamUnavailable(reason) }
}

/// How fast each team of an org ships, over a period the person chooses.
@MainActor
@Observable
public final class EvaluationStore {
    public enum Loading: Equatable, Sendable {
        case idle
        case loading
        case loaded
        case failed(String)
    }

    /// The periods to choose from, in days.
    public static let periods = [14, 30, 90]
    public static let defaultDays = 14

    public private(set) var loading: Loading = .idle
    /// Whether a read has finished at least once. Set once and never reset: a refresh under way, a failed one or a cancelled one
    /// does not make the screen unread again, so an account with nothing in it keeps saying so instead of going back to bars.
    public private(set) var hasAnswered = false
    /// The Access session ended: the sign-in has to run again before anything else works.
    public private(set) var needsSignIn = false
    public private(set) var orgs: [OrgRef] = []
    /// The org shown: the one chosen, or the first of the account's orgs.
    public private(set) var org: OrgRef?
    public private(set) var days = EvaluationStore.defaultDays
    public private(set) var state: EvaluationState = .loading
    public private(set) var effort: EffortState = .loading
    /// How each team's week stands against its weekly budget, by team: what the node says, or why there is none to show.
    public private(set) var budgets: [String: BudgetRead] = [:]

    /// The node's own response times, shown at the end of the screen for the owner; `nil` for a store that has no node to ask.
    public let nodeSpeed: NodeSpeedStore?

    @ObservationIgnored private let source: any EvaluationSource
    @ObservationIgnored private let retry: RetryPolicy
    @ObservationIgnored private var chosenOrg: String?

    public init(source: any EvaluationSource, retry: RetryPolicy = .standard, nodeSpeed: NodeSpeedStore? = nil) {
        self.source = source
        self.retry = retry
        self.nodeSpeed = nodeSpeed
    }

    // MARK: Reading

    public func refresh() async {
        let before = loading
        loading = .loading
        do {
            let me = try await source.me()
            orgs = me.orgs
            org = me.orgs.first { $0.id == chosenOrg } ?? me.orgs.first
            needsSignIn = false
            await readOrg()
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

    public func choose(org id: String) async {
        guard id != org?.id, let next = orgs.first(where: { $0.id == id }) else { return }
        chosenOrg = id
        org = next
        state = .loading
        effort = .loading
        budgets = [:]
        await readOrg()
    }

    public func choose(days: Int) async {
        guard Self.periods.contains(days), days != self.days else { return }
        self.days = days
        await readOrg()
    }

    /// What one read of the node came to.
    private enum Fetched<Value: Sendable>: Sendable {
        case answer(Value)
        case unavailable
        case couldNotAnswer(String)
        case failed(String)
        case cancelled
    }

    private static func fetch<Value: Sendable>(_ retry: RetryPolicy, _ read: @Sendable () async throws -> Value) async -> Fetched<Value> {
        do {
            return .answer(try await retry.run(read))
        } catch NodeError.http(404, _) {
            return .unavailable
        } catch NodeError.http(503, let message) {
            return .couldNotAnswer(message ?? "HTTP 503")
        } catch NodeError.unreadable {
            // The answer is not what the route promises: the same as the team's runtime not answering.
            return .couldNotAnswer(Ja.Evaluation.unreadable)
        } catch is CancellationError {
            return .cancelled
        } catch {
            return .failed(DeskStore.reason(error))
        }
    }

    /// Reads the evaluation, the effort and the org's plans together.
    private func readOrg() async {
        guard let org else {
            state = .loading
            return
        }
        let (id, days, source, retry) = (org.id, days, source, retry)
        async let evaluation = Self.fetch(retry) { try await source.evaluation(org: id, days: days) }
        async let effort = Self.fetch(retry) { try await source.effort(org: id, days: days) }
        async let plans = Self.fetch(retry) { try await source.orgPlans(org: id) }
        let (read, spent, intended) = await (evaluation, effort, plans)
        // A period or an org chosen meanwhile has its own answer coming; this one is for the old one.
        guard id == self.org?.id, days == self.days else { return }
        apply(read, in: org)
        apply(spent, plans: intended, in: org)
        await readBudgets(in: org, days: days)
    }

    // MARK: The weekly budget

    /// Asks the node how each team of the effort stands against its weekly budget, once the effort says which teams there are. A node that does not
    /// list the capability `budget` is not asked, and says only what the plan states; the app works out no week and no threshold of its own.
    private func readBudgets(in org: OrgRef, days: Int) async {
        guard case .ready(let reports, _) = effort else { return }
        for report in reports where budgets[report.team] == nil { budgets[report.team] = .loading }
        let (id, source, retry) = (org.id, source, retry)
        let about = await Self.fetch(retry) { try await source.about() }
        let read: [String: Fetched<BudgetStatus>]
        switch about {
        case .answer(let said) where said.has(.budget):
            read = await Self.budgets(of: reports.map(\.team), in: id, source: source, retry: retry)
        case .answer, .unavailable:
            // A node that has no `about`, or does not list the capability, is not asked.
            read = Dictionary(uniqueKeysWithValues: reports.map { ($0.team, Fetched<BudgetStatus>.unavailable) })
        case .couldNotAnswer(let reason), .failed(let reason):
            read = Dictionary(uniqueKeysWithValues: reports.map { ($0.team, Fetched<BudgetStatus>.failed(reason)) })
        case .cancelled:
            return
        }
        guard id == self.org?.id, days == self.days else { return }
        for report in reports {
            guard let one = read[report.team] else { continue }
            apply(one, to: report)
        }
    }

    private static func budgets(
        of teams: [String], in org: String, source: any EvaluationSource, retry: RetryPolicy
    ) async -> [String: Fetched<BudgetStatus>] {
        await withTaskGroup(of: (String, Fetched<BudgetStatus>).self) { group in
            for team in teams {
                group.addTask { (team, await Self.fetch(retry) { try await source.budget(org: org, team: team) }) }
            }
            var all: [String: Fetched<BudgetStatus>] = [:]
            for await (team, one) in group { all[team] = one }
            return all
        }
    }

    private func apply(_ read: Fetched<BudgetStatus>, to report: TeamEffortReport) {
        switch read {
        case .answer(let status): budgets[report.team] = .reported(BudgetReport(status))
        case .unavailable: budgets[report.team] = report.plannedBudget.map { .plain(Ja.Effort.budgetPlain(BudgetReport.money(Double($0)))) } ?? BudgetRead.none
        // What is on screen stays when a refresh fails.
        case .couldNotAnswer(let reason), .failed(let reason):
            if case .reported = budgets[report.team] { return }
            budgets[report.team] = .failed(reason)
        case .cancelled: return
        }
    }

    private func apply(_ read: Fetched<Evaluation>, in org: OrgRef) {
        let outcome: EvaluationState
        switch read {
        case .answer(let answer): outcome = .ready(reports(answer, of: org), unavailable: notes(answer.unavailable, of: org))
        case .unavailable: outcome = .unavailable
        case .couldNotAnswer(let reason): outcome = .couldNotAnswer(reason)
        case .failed(let reason): outcome = .failed(reason)
        case .cancelled: return
        }
        // What is on screen stays when a refresh keeps failing.
        if case .ready = state, outcome.isRetryExhausted { return }
        state = outcome
    }

    private func apply(_ read: Fetched<Effort>, plans: Fetched<OrgPlans>, in org: OrgRef) {
        let outcome: EffortState
        switch read {
        case .answer(let answer):
            let planFor: (String) -> PlanState = { team in
                guard case .answer(let known) = plans else { return .unreadable }
                return known.byTeam[team].map(PlanState.stated) ?? .none
            }
            let reports = answer.teams.map { TeamEffortReport(effort: $0, label: label(of: $0.team, in: org), days: answer.days, plan: planFor($0.team)) }
            outcome = .ready(reports, unavailable: notes(answer.unavailable, of: org))
        case .unavailable: outcome = .unavailable
        case .couldNotAnswer(let reason): outcome = .couldNotAnswer(reason)
        case .failed(let reason): outcome = .failed(reason)
        case .cancelled: return
        }
        if case .ready = effort, outcome.isRetryExhausted { return }
        effort = outcome
    }

    private func label(of team: String, in org: OrgRef) -> String {
        org.teams.first { $0.id == team }?.label ?? team
    }

    private func reports(_ answer: Evaluation, of org: OrgRef) -> [TeamEvaluationReport] {
        answer.teams.map { TeamEvaluationReport(numbers: $0, label: label(of: $0.team, in: org), days: answer.days) }
    }

    private func notes(_ unavailable: [TeamUnavailable], of org: OrgRef) -> [TeamUnavailableNote] {
        unavailable.map { TeamUnavailableNote(team: $0.team, label: label(of: $0.team, in: org), reason: $0.reason) }
    }
}

extension EvaluationState {
    var isRetryExhausted: Bool {
        switch self {
        case .couldNotAnswer, .failed: true
        case .loading, .ready, .unavailable: false
        }
    }
}
