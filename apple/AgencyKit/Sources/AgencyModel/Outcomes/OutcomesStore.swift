import AgencyAPI
import Foundation
import Observation

/// Where the outcomes page reads from: the home node's Client API. `NodeClient` is the real one.
public protocol OutcomesSource: Sendable {
    func me() async throws -> Me
    func outcomes(org: String, days: Int) async throws -> Outcomes
    func readings(org: String) async throws -> Readings
    /// The evaluation of the last `days` days, for the cost under the KPIs.
    func evaluation(org: String, days: Int) async throws -> Evaluation
    func enterNumber(org: String, input: ObservationInput) async throws -> KpiObservation
    func numberHistory(org: String, kpi: String, limit: Int) async throws -> [KpiObservation]
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

/// What is known of the history of one KPI the owner has opened.
public enum HistoryState: Equatable, Sendable {
    case loading
    case entries([NumberHistoryEntry])
    case failed(String)
}

/// What came of entering a number.
public enum EntryOutcome: Equatable, Sendable {
    /// Saved: what to say, and the numbers and readings were read again.
    case saved(String)
    /// What was typed is not one the node takes: said before anything was sent.
    case problem(NumberProblem)
    /// The node did not take it, or could not be asked: the words for the owner.
    case failed(String)
}

/// One org's numbers as the page draws them.
public struct OrgOutcomes: Equatable, Identifiable, Sendable {
    public let id: String
    public let label: String
    /// The period's first and last calendar day in the node's time zone, which a trend is drawn between.
    public let from: String
    public let until: String
    public let cards: [KpiCard]
    public let readings: OrgReadings

    public init(id: String, label: String, from: String, until: String, cards: [KpiCard], readings: OrgReadings) {
        self.id = id
        self.label = label
        self.from = from
        self.until = until
        self.cards = cards
        self.readings = readings
    }
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

    /// The history of a KPI the owner has opened, by `org/kpi`: kept only while it is open, in memory like the rest.
    public private(set) var histories: [String: HistoryState] = [:]

    /// What the org's work cost on the days its KPIs are drawn over, by org, for the period shown. An org not in it has not been
    /// read: its cost is on its way, which is not a cost of nothing. It is read after the numbers are on screen (an evaluation asks
    /// every team's runtime, which is slow) and again no sooner than `costLifetime` later.
    public private(set) var costs: [String: OrgCost] = [:]

    /// How long a read cost is kept before a refresh asks again.
    public static let costLifetime: TimeInterval = 300
    @ObservationIgnored private var costsReadAt: [String: Date] = [:]
    /// The cost reads under way, by org: each is the in-flight marker of its org.
    @ObservationIgnored private var costTasks: [String: Task<Void, Never>] = [:]
    /// Which period's cost reads the markers belong to; another period starts a new one.
    @ObservationIgnored private var costGeneration = 0
    @ObservationIgnored private let now: @Sendable () -> Date

    @ObservationIgnored private let source: any OutcomesSource
    @ObservationIgnored private let retry: RetryPolicy

    public init(source: any OutcomesSource, retry: RetryPolicy = .standard, now: @escaping @Sendable () -> Date = { Date() }) {
        self.source = source
        self.retry = retry
        self.now = now
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
            startCostReads()
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

    /// Starts the read of the cost of each org that needs one, in a task of its own, and returns at once: an evaluation asks every team's
    /// runtime and is slow, so the numbers and the refresh that brought them are not held back for it. An org whose cost is being read
    /// is not asked again by a second refresh (the task is its in-flight marker), one whose cost was read lately is not asked either, and
    /// one whose cost could not be read is asked again. Another period cancels what is under way.
    private func startCostReads() {
        let (source, retry, stamp, generation) = (source, retry, now(), costGeneration)
        for org in orgs where costTasks[org.id] == nil && needsCost(org, at: stamp) {
            costTasks[org.id] = Task { [weak self] in
                let cost = await Self.readCost(of: org, source: source, retry: retry)
                self?.finishCost(of: org.id, cost, generation: generation, at: stamp)
            }
        }
    }

    private func needsCost(_ org: OrgOutcomes, at stamp: Date) -> Bool {
        // Only a cost that was read is kept: one that could not be read is asked for again.
        guard case .ready? = costs[org.id], let readAt = costsReadAt[org.id] else { return true }
        return stamp.timeIntervalSince(readAt) >= Self.costLifetime
    }

    private func finishCost(of id: String, _ cost: OrgCost?, generation: Int, at stamp: Date) {
        // A period chosen meanwhile has its own cost coming (and its own markers): what is left of the old one touches nothing.
        guard generation == costGeneration else { return }
        costTasks[id] = nil
        // A read that was cancelled says nothing.
        guard let cost else { return }
        costs[id] = cost
        costsReadAt[id] = stamp
    }

    /// Waits for the costs that are being read: for a test, or a screen that has to show them before it goes on.
    public func waitForCosts() async {
        // A task removes its marker when it finishes.
        while let task = costTasks.values.first { await task.value }
    }

    /// `nil` when the read was cancelled, which says nothing.
    private static func readCost(of org: OrgOutcomes, source: any OutcomesSource, retry: RetryPolicy) async -> OrgCost? {
        let dates = OutcomeDay.dates(from: org.from, until: org.until)
        guard !dates.isEmpty else { return .unavailable }
        do {
            let evaluation = try await retry.run { try await source.evaluation(org: org.id, days: dates.count) }
            return .ready(CostOnDays(evaluation: evaluation, dates: dates))
        } catch is CancellationError {
            return nil
        } catch {
            return .unavailable
        }
    }

    /// The key of a KPI's history in `histories`.
    public static func historyKey(org: String, kpi: String) -> String { "\(org)/\(kpi)" }

    /// Enters a number of a KPI for a day. What was typed is checked as the node checks it first, with the node's day as today;
    /// then the node is asked once (a write is never repeated blindly), its refusals are said in words, and after a save the
    /// numbers and the readings are read again, and so is every history that is open.
    public func enter(org: String, kpi: Kpi, form: NumberForm) async -> EntryOutcome {
        guard let today = orgs.first(where: { $0.id == org })?.until else { return .failed(Ja.Desk.loadFailed("")) }
        let input: ObservationInput
        switch NumberCheck.check(form, kpi: kpi.id, today: today) {
        case .failure(let problem): return .problem(problem)
        case .success(let checked): input = checked
        }
        do {
            let saved = try await source.enterNumber(org: org, input: input)
            await refresh()
            for key in histories.keys { await loadHistory(key: key, unit: kpi.unit, until: today) }
            return .saved(Ja.Outcomes.Enter.saved(OutcomeFormat.value(saved.value, unit: kpi.unit), day: OutcomeFormat.day(saved.date, relativeTo: today)))
        } catch NodeError.sessionExpired, NodeError.notSignedIn {
            needsSignIn = true
            return .failed(Ja.Desk.signInEnded)
        } catch NodeError.http(409, _) {
            return .failed(Ja.Outcomes.Enter.full)
        } catch NodeError.http(403, _) {
            return .failed(Ja.Outcomes.Enter.forbidden)
        } catch NodeError.http(let status, let message) where status < 500 {
            return .failed(Ja.Outcomes.Enter.refused(message ?? "HTTP \(status)"))
        } catch {
            return .failed(DeskStore.reason(error))
        }
    }

    /// Opens the history of a KPI and reads it.
    public func showHistory(org: String, kpi: Kpi) async {
        let key = Self.historyKey(org: org, kpi: kpi.id)
        histories[key] = .loading
        await loadHistory(key: key, unit: kpi.unit, until: orgs.first(where: { $0.id == org })?.until ?? "")
    }

    public func hideHistory(org: String, kpi: String) {
        histories[Self.historyKey(org: org, kpi: kpi)] = nil
    }

    private func loadHistory(key: String, unit: Kpi.Unit, until: String) async {
        let parts = key.split(separator: "/", maxSplits: 1).map(String.init)
        guard parts.count == 2 else { return }
        do {
            let entries = try await source.numberHistory(org: parts[0], kpi: parts[1], limit: 100)
            // Closed meanwhile: nothing to show.
            guard histories[key] != nil else { return }
            histories[key] = .entries(NumberHistoryEntry.list(entries, unit: unit, until: until))
        } catch is CancellationError {
            return
        } catch {
            if histories[key] != nil { histories[key] = .failed(DeskStore.reason(error)) }
        }
    }

    public func choose(days: Int) async {
        guard Self.periods.contains(days), days != self.days else { return }
        self.days = days
        costTasks.values.forEach { $0.cancel() }
        costTasks = [:]
        costGeneration += 1
        costs = [:]
        costsReadAt = [:]
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
        return .answer(OrgOutcomes(
            id: id, label: org.label, from: outcomes.from, until: outcomes.until,
            cards: outcomes.series.map { KpiCard(series: $0, until: outcomes.until) }, readings: readings
        ))
    }

    private static func readReadings(_ org: OrgRef, kpis: [Kpi], source: any OutcomesSource, retry: RetryPolicy) async -> OrgReadings {
        let id = org.id
        do {
            let answer = try await retry.run { try await source.readings(org: id) }
            let kpiOf = Dictionary(kpis.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
            let teamLabel = Dictionary(org.teams.map { ($0.id, $0.label) }, uniquingKeysWith: { first, _ in first })
            let today = String(answer.generatedAt.text.prefix(10))
            return .lines(answer.readings.map { ReadingLine(reading: $0, team: teamLabel[$0.team] ?? $0.team, kpi: kpiOf[$0.kpi], today: today) })
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
