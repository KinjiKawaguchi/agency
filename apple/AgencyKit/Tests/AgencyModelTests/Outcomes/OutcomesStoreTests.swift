import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

private final class FakeOutcomesSource: OutcomesSource, @unchecked Sendable {
    private let lock = NSLock()
    private var _me: Result<Me, Error> = .success(try! JSONDecoder().decode(Me.self, from: Data(FakeOutcomesSource.twoOrgs.utf8)))
    private var _outcomes: @Sendable (String, Int) throws -> Outcomes = { _, _ in try OutcomeSamples.outcomes() }
    private var _readings: @Sendable (String) throws -> Readings = { _ in try OutcomeSamples.readings() }
    private var gate: CheckedContinuation<Void, Never>?
    private var holdsNext = false
    private(set) var daysAsked: [Int] = []
    private var _enter: @Sendable (ObservationInput) throws -> KpiObservation = { input in
        let json = #"{"kpi":"\#(input.kpi)","date":"\#(input.date)","value":\#(input.value),"source":{"kind":"manual"},"enteredAt":"2026-11-24T12:00:00Z","enteredBy":"owner"}"#
        return try JSONDecoder().decode(KpiObservation.self, from: Data(json.utf8))
    }
    private var _history: @Sendable (String, String) throws -> [KpiObservation] = { _, _ in [] }
    private(set) var entered: [ObservationInput] = []
    private(set) var historyReads = 0
    private var _evaluation: @Sendable (String, Int) throws -> Evaluation = { _, _ in throw NodeError.http(status: 404, message: nil) }
    private(set) var evaluationsAsked: [(org: String, days: Int)] = []
    private var holdsEvaluations = false
    /// How many evaluations found their task cancelled by the time they were let go on.
    private(set) var cancelledEvaluations = 0
    private var evaluationGates: [CheckedContinuation<Void, Never>] = []

    static let twoOrgs = """
    {"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[
    {"id":"acme","isRoot":true,"label":"Acme","teams":[{"id":"app","label":"App","location":"local","viewer":"owner"}]},
    {"id":"lab","isRoot":false,"label":"Lab","teams":[]}]}
    """

    func set(me: Result<Me, Error>) { lock.withLock { _me = me } }
    func set(outcomes: @escaping @Sendable (String, Int) throws -> Outcomes) { lock.withLock { _outcomes = outcomes } }
    func set(readings: @escaping @Sendable (String) throws -> Readings) { lock.withLock { _readings = readings } }
    func set(enter: @escaping @Sendable (ObservationInput) throws -> KpiObservation) { lock.withLock { _enter = enter } }
    func set(evaluation: @escaping @Sendable (String, Int) throws -> Evaluation) { lock.withLock { _evaluation = evaluation } }
    func set(history: @escaping @Sendable (String, String) throws -> [KpiObservation]) { lock.withLock { _history = history } }

    func enterNumber(org: String, input: ObservationInput) async throws -> KpiObservation {
        let make = lock.withLock { entered.append(input); return _enter }
        return try make(input)
    }

    func numberHistory(org: String, kpi: String, limit: Int) async throws -> [KpiObservation] {
        let make = lock.withLock { historyReads += 1; return _history }
        return try make(org, kpi)
    }
    func holdNext() { lock.withLock { holdsNext = true } }
    func release() { lock.withLock { let waiting = gate; gate = nil; return waiting }?.resume() }
    var isHolding: Bool { lock.withLock { gate != nil } }

    func me() async throws -> Me { try lock.withLock { try _me.get() } }

    func outcomes(org: String, days: Int) async throws -> Outcomes {
        if lock.withLock({ let held = holdsNext; holdsNext = false; return held }) {
            await withCheckedContinuation { continuation in lock.withLock { gate = continuation } }
        }
        let make = lock.withLock { daysAsked.append(days); return _outcomes }
        return try make(org, days)
    }

    func evaluation(org: String, days: Int) async throws -> Evaluation {
        let (make, holds) = lock.withLock { evaluationsAsked.append((org, days)); return (_evaluation, holdsEvaluations) }
        if holds { await withCheckedContinuation { continuation in lock.withLock { evaluationGates.append(continuation) } } }
        if Task.isCancelled { lock.withLock { cancelledEvaluations += 1 } }
        return try make(org, days)
    }

    /// From now on an evaluation waits until it is released, in the order it was asked.
    func holdEvaluations() { lock.withLock { holdsEvaluations = true } }
    var heldEvaluations: Int { lock.withLock { evaluationGates.count } }

    /// Lets the first `count` held evaluations go on (all of them by default), and no later one is held.
    func releaseEvaluations(_ count: Int? = nil) {
        let released = lock.withLock { () -> [CheckedContinuation<Void, Never>] in
            let take = Array(evaluationGates.prefix(count ?? evaluationGates.count))
            evaluationGates.removeFirst(take.count)
            if count == nil { holdsEvaluations = false }
            return take
        }
        released.forEach { $0.resume() }
    }

    func readings(org: String) async throws -> Readings {
        let make = lock.withLock { _readings }
        return try make(org)
    }
}

@MainActor
@Suite("OutcomesStore")
struct OutcomesStoreTests {
    private let source = FakeOutcomesSource()

    private func makeStore() -> OutcomesStore {
        OutcomesStore(source: source, retry: .never)
    }

    private let forbidden = NodeError.http(status: 403, message: "the numbers are for the owner")

    @Test("the owner's answer gives the page: a card for each KPI and a line for each aim, and the page is offered")
    func owner() async throws {
        let store = makeStore()
        source.set(outcomes: { org, _ in
            if org == "lab" { throw NodeError.http(status: 403, message: nil) }
            return try OutcomeSamples.outcomes()
        })
        await store.refresh()
        #expect(store.loading == .loaded && store.hasAnswered && store.isAvailable)
        #expect(store.orgs.map(\.id) == ["acme"], "the org the owner may not read is not there, and nothing says so")
        let acme = try #require(store.orgs.first)
        #expect(acme.cards.map(\.label) == ["Monthly revenue", "Activation", "Lead time of a feature"])
        guard case .lines(let lines) = acme.readings else { Issue.record("no readings"); return }
        #expect(lines.count == 8)
        #expect(lines[0].team == "App", "the team's own label")
        #expect(acme.from == "2026-11-11" && acme.until == "2026-11-24", "the period the trends are drawn between")
        #expect(lines[0].headline.contains("10月15日") && !lines[0].headline.contains("年"), "short days: the readings are of the year of their day")
    }

    @Test("when every org refuses, there is nothing to show, no failure, and the page is not offered")
    func notOwner() async {
        source.set(outcomes: { _, _ in throw NodeError.http(status: 403, message: nil) })
        let store = makeStore()
        await store.refresh()
        #expect(store.loading == .loaded)
        #expect(store.orgs.isEmpty)
        #expect(!store.isAvailable)
        #expect(store.hasAnswered)
    }

    @Test("a node that does not know the org (404) is refused the same way, without a word")
    func notFound() async {
        source.set(outcomes: { _, _ in throw NodeError.http(status: 404, message: nil) })
        let store = makeStore()
        await store.refresh()
        #expect(!store.isAvailable && store.orgs.isEmpty && store.loading == .loaded)
    }

    @Test("before any answer the page is not offered, and a read that fails before one ever worked does not offer it")
    func notOfferedBeforeAnswer() async {
        let store = makeStore()
        #expect(!store.isAvailable && !store.hasAnswered && store.loading == .idle)
        source.set(outcomes: { _, _ in throw NodeError.http(status: 500, message: "broken") })
        await store.refresh()
        #expect(store.loading == .failed("broken"))
        #expect(!store.isAvailable && !store.hasAnswered)
    }

    @Test("once it worked the page stays: a later failure keeps what was read and says it failed")
    func stays() async {
        let store = makeStore()
        await store.refresh()
        #expect(store.isAvailable && store.orgs.count == 2)
        source.set(outcomes: { _, _ in throw NodeError.http(status: 503, message: "down") })
        await store.refresh()
        #expect(store.loading == .failed("down"))
        #expect(store.isAvailable && store.hasAnswered && store.orgs.count == 2)
    }

    @Test("a refresh under way keeps the answer and the offer")
    func underWay() async {
        let store = makeStore()
        await store.refresh()
        source.holdNext()
        let refreshing = Task { await store.refresh() }
        while !source.isHolding { await Task.yield() }
        #expect(store.loading == .loading)
        #expect(store.isAvailable && store.hasAnswered && !store.orgs.isEmpty)
        source.release()
        await refreshing.value
        #expect(store.loading == .loaded)
    }

    @Test("when the owner is later refused everywhere the page is no longer offered")
    func refusedLater() async {
        let store = makeStore()
        await store.refresh()
        source.set(outcomes: { _, _ in throw NodeError.http(status: 403, message: nil) })
        await store.refresh()
        #expect(!store.isAvailable && store.orgs.isEmpty)
    }

    @Test("the readings failing does not take the cards away, and says so")
    func readingsFail() async throws {
        source.set(readings: { _ in throw NodeError.http(status: 500, message: "no readings") })
        let store = makeStore()
        await store.refresh()
        let acme = try #require(store.orgs.first)
        #expect(!acme.cards.isEmpty)
        #expect(acme.readings == .failed("no readings"))
    }

    @Test("a node that does not serve the readings (404) has the cards and no readings")
    func readingsUnavailable() async throws {
        source.set(readings: { _ in throw NodeError.http(status: 404, message: nil) })
        let store = makeStore()
        await store.refresh()
        #expect(try #require(store.orgs.first).readings == .unavailable)
    }

    @Test("the period is 90 days to begin with, may be 30 or 90, and a choice reads again")
    func period() async {
        let store = makeStore()
        #expect(store.days == 90 && OutcomesStore.periods == [30, 90])
        await store.refresh()
        await store.choose(days: 14)
        #expect(store.days == 90, "a period the page does not offer is ignored")
        await store.choose(days: 30)
        #expect(store.days == 30)
        #expect(source.daysAsked.suffix(2) == [30, 30])
        #expect(source.daysAsked.first == 90)
    }

    @Test("an ended sign-in is asked for again, and not shown as a failure of the numbers")
    func signIn() async {
        source.set(me: .failure(NodeError.sessionExpired))
        let store = makeStore()
        await store.refresh()
        #expect(store.needsSignIn)
        #expect(!store.isAvailable)
    }

    @Test("a read that is cancelled after the page answered leaves it as it was: the numbers do not turn back into bars")
    func cancelledKeepsTheAnswer() async {
        let store = makeStore()
        await store.refresh()
        #expect(store.loading == .loaded)
        source.set(outcomes: { _, _ in throw CancellationError() })
        await store.refresh()
        #expect(store.loading == .loaded && store.isAvailable && store.hasAnswered && !store.orgs.isEmpty)
    }

    @Test("the same when it is the account's read that is cancelled")
    func cancelledAccountRead() async {
        let store = makeStore()
        await store.refresh()
        source.set(me: .failure(CancellationError()))
        await store.refresh()
        #expect(store.loading == .loaded && store.isAvailable && !store.orgs.isEmpty)
    }

    @Test("a first read that is cancelled stays as it was before it, not answered")
    func cancelledFirstRead() async {
        source.set(outcomes: { _, _ in throw CancellationError() })
        let store = makeStore()
        await store.refresh()
        #expect(store.loading == .idle && !store.hasAnswered && !store.isAvailable)
    }

    @Test("a session that ends in the middle of the read asks for the sign-in again, and is not a failure of the numbers")
    func signInEndsInTheMiddle() async {
        let store = makeStore()
        await store.refresh()
        source.set(outcomes: { _, _ in throw NodeError.sessionExpired })
        await store.refresh()
        #expect(store.needsSignIn)
        #expect(store.loading == .failed(Ja.Desk.signInEnded))
        #expect(store.isAvailable && !store.orgs.isEmpty, "what was read stays")
        source.set(outcomes: { _, _ in throw NodeError.notSignedIn })
        await store.refresh()
        #expect(store.needsSignIn)
        source.set(outcomes: { _, _ in try OutcomeSamples.outcomes() })
        await store.refresh()
        #expect(!store.needsSignIn && store.loading == .loaded)
    }

    // MARK: Entering a number

    private let revenue = try! OutcomeSamples.outcomes().series[0].kpi

    @Test("a number that is saved is checked first, sent once with the node's day as today, said in words, and the numbers are read again")
    func enterSaves() async throws {
        let store = makeStore()
        await store.refresh()
        let readsBefore = source.daysAsked.count
        let outcome = await store.enter(org: "acme", kpi: revenue, form: NumberForm(date: "2026-11-24", value: "1400000"))
        guard case .saved(let words) = outcome else { Issue.record("not saved: \(outcome)"); return }
        #expect(words.contains("1,400,000") && words.contains("11月24日") && words.hasSuffix("を保存しました。"))
        #expect(source.entered == [ObservationInput(kpi: "revenue", date: "2026-11-24", value: 1_400_000)])
        #expect(source.daysAsked.count > readsBefore, "the numbers are read again after a save")
    }

    @Test("what is not a number the node takes is said before anything is sent")
    func enterProblems() async {
        let store = makeStore()
        await store.refresh()
        #expect(await store.enter(org: "acme", kpi: revenue, form: NumberForm(date: "2026-11-30", value: "1")) == .problem(.date))
        #expect(await store.enter(org: "acme", kpi: revenue, form: NumberForm(date: "2026-11-24", value: "1e3")) == .problem(.value))
        #expect(await store.enter(org: "acme", kpi: revenue, form: NumberForm(date: "2026-11-24", value: "1", note: "a\nb")) == .problem(.note))
        #expect(source.entered.isEmpty, "nothing was sent")
    }

    @Test("the node's refusals are said in words: full, not the owner, the node's own reason, and a failure to reach it")
    func enterRefusals() async {
        let store = makeStore()
        await store.refresh()
        let form = NumberForm(date: "2026-11-24", value: "1")
        for (status, message, expected) in [
            (409, "full", Ja.Outcomes.Enter.full), (403, "no", Ja.Outcomes.Enter.forbidden),
            (400, "the day is in the future", Ja.Outcomes.Enter.refused("the day is in the future")),
            (404, "no such kpi", Ja.Outcomes.Enter.refused("no such kpi")),
        ] {
            source.set(enter: { _ in throw NodeError.http(status: status, message: message) })
            #expect(await store.enter(org: "acme", kpi: revenue, form: form) == .failed(expected), Comment(rawValue: "\(status)"))
        }
        source.set(enter: { _ in throw NodeError.http(status: 503, message: "down") })
        #expect(await store.enter(org: "acme", kpi: revenue, form: form) == .failed("down"))
        source.set(enter: { _ in throw NodeError.http(status: 400, message: nil) })
        #expect(await store.enter(org: "acme", kpi: revenue, form: form) == .failed(Ja.Outcomes.Enter.refused("HTTP 400")))
        let readsBefore = source.daysAsked.count
        #expect(source.entered.count == 6)
        #expect(source.daysAsked.count == readsBefore, "a refusal reads nothing again")
    }

    @Test("a session that ended while saving asks for the sign-in again, and a failed save does not hide what was shown")
    func enterSignIn() async {
        let store = makeStore()
        await store.refresh()
        source.set(enter: { _ in throw NodeError.sessionExpired })
        #expect(await store.enter(org: "acme", kpi: revenue, form: NumberForm(date: "2026-11-24", value: "1")) == .failed(Ja.Desk.signInEnded))
        #expect(store.needsSignIn && store.isAvailable && !store.orgs.isEmpty)
    }

    @Test("an org the page does not show cannot be entered into")
    func enterUnknownOrg() async {
        let store = makeStore()
        await store.refresh()
        let outcome = await store.enter(org: "nowhere", kpi: revenue, form: NumberForm(date: "2026-11-24", value: "1"))
        guard case .failed = outcome else { Issue.record("not refused: \(outcome)"); return }
        #expect(source.entered.isEmpty)
    }

    @Test("a history is read when opened, listed with what a later entry replaced, kept only while open, and read again after a save")
    func history() async throws {
        let store = makeStore()
        await store.refresh()
        func entry(_ date: String, _ value: Double, _ at: String) throws -> KpiObservation {
            try JSONDecoder().decode(KpiObservation.self, from: Data(#"{"kpi":"revenue","date":"\#(date)","value":\#(value),"source":{"kind":"manual"},"enteredAt":"\#(at)","enteredBy":"owner"}"#.utf8))
        }
        let list = [try entry("2026-11-20", 2, "2026-11-22T09:00:00Z"), try entry("2026-11-20", 1, "2026-11-21T09:00:00Z")]
        source.set(history: { _, _ in list })
        let key = OutcomesStore.historyKey(org: "acme", kpi: "revenue")
        #expect(store.histories[key] == nil)
        await store.showHistory(org: "acme", kpi: revenue)
        guard case .entries(let shown)? = store.histories[key] else { Issue.record("no entries"); return }
        #expect(shown.map(\.isReplaced) == [false, true])
        let reads = source.historyReads
        _ = await store.enter(org: "acme", kpi: revenue, form: NumberForm(date: "2026-11-24", value: "5"))
        #expect(source.historyReads == reads + 1, "an open history is read again after a save")
        store.hideHistory(org: "acme", kpi: "revenue")
        #expect(store.histories[key] == nil)
        _ = await store.enter(org: "acme", kpi: revenue, form: NumberForm(date: "2026-11-24", value: "6"))
        #expect(source.historyReads == reads + 1, "a closed history is not read")
    }

    @Test("a history that cannot be read says why, and one closed meanwhile shows nothing")
    func historyFails() async {
        let store = makeStore()
        await store.refresh()
        source.set(history: { _, _ in throw NodeError.http(status: 503, message: "down") })
        await store.showHistory(org: "acme", kpi: revenue)
        #expect(store.histories[OutcomesStore.historyKey(org: "acme", kpi: "revenue")] == .failed("down"))
    }

    @Test("an account with no orgs has answered with nothing, and the page is not offered")
    func noOrgs() async throws {
        let none = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[]}"#
        source.set(me: .success(try JSONDecoder().decode(Me.self, from: Data(none.utf8))))
        let store = makeStore()
        await store.refresh()
        #expect(store.loading == .loaded && store.hasAnswered && !store.isAvailable)
    }

    // MARK: The cost of the same days

    private final class Clock: @unchecked Sendable {
        private let lock = NSLock()
        private var _now = Date(timeIntervalSince1970: 1_795_000_000)
        var now: Date { lock.withLock { _now } }
        func advance(_ seconds: TimeInterval) { lock.withLock { _now = _now.addingTimeInterval(seconds) } }
    }

    private func costStore(_ clock: Clock = Clock()) -> OutcomesStore {
        source.set(outcomes: { _, _ in try OutcomeSamples.outcomes() })
        source.set(evaluation: { _, _ in try CostSamples.evaluation(teams: [(from: "2026-11-11", until: "2026-11-24", byDay: [(date: "2026-11-23", usd: 9.5)])]) })
        return OutcomesStore(source: source, retry: .never, now: { clock.now })
    }

    @Test("the cost of the days of the KPIs is read for each org after the numbers, for as many days as the KPIs are drawn over")
    func costRead() async throws {
        let store = costStore()
        #expect(store.costs.isEmpty, "not read yet is not a cost of nothing")
        await store.refresh()
        await store.waitForCosts()
        #expect(store.loading == .loaded && store.orgs.count == 2)
        #expect(Set(source.evaluationsAsked.map(\.org)) == ["acme", "lab"] && source.evaluationsAsked.allSatisfy { $0.days == 14 })
        guard case .ready(let cost)? = store.costs["acme"] else { Issue.record("no cost"); return }
        #expect(cost.days.count == 14 && cost.days.first?.date == "2026-11-11" && cost.days.last?.date == "2026-11-24")
        #expect(cost.days.first { $0.date == "2026-11-23" }?.usd == 9.5 && cost.days.first { $0.date == "2026-11-12" }?.usd == 0)
    }

    @Test("a cost that cannot be read says so for its org and leaves the numbers alone, and is asked for again at the next read")
    func costFails() async throws {
        let store = costStore()
        source.set(evaluation: { _, _ in throw NodeError.http(status: 503, message: "busy") })
        await store.refresh()
        await store.waitForCosts()
        #expect(store.loading == .loaded && store.orgs.count == 2 && store.costs["acme"] == .unavailable)
        let asked = source.evaluationsAsked.count
        source.set(evaluation: { _, _ in try CostSamples.evaluation(teams: [(from: "2026-11-11", until: "2026-11-24", byDay: [])]) })
        await store.refresh()
        await store.waitForCosts()
        #expect(source.evaluationsAsked.count == asked + 2)
        if case .ready? = store.costs["acme"] {} else { Issue.record("should be ready now") }
    }

    @Test("a cost that was read is kept for five minutes, then asked for again")
    func costLifetime() async {
        let clock = Clock()
        let store = costStore(clock)
        await store.refresh()
        await store.waitForCosts()
        let first = source.evaluationsAsked.count
        clock.advance(OutcomesStore.costLifetime - 1)
        await store.refresh()
        await store.waitForCosts()
        #expect(source.evaluationsAsked.count == first, "a refresh every few seconds does not ask every team's runtime each time")
        clock.advance(2)
        await store.refresh()
        await store.waitForCosts()
        #expect(source.evaluationsAsked.count == first + 2)
    }

    @Test("another period has its own cost: the one read is dropped and the new one is read for the new days")
    func costPeriod() async {
        let store = costStore()
        await store.refresh()
        await store.waitForCosts()
        let first = source.evaluationsAsked.count
        await store.choose(days: 30)
        await store.waitForCosts()
        #expect(source.evaluationsAsked.count == first + 2)
        #expect(store.costs.count == 2)
    }

    @Test("a cost read that is cancelled says nothing for its org")
    func costCancelled() async {
        let store = costStore()
        source.set(evaluation: { _, _ in throw CancellationError() })
        await store.refresh()
        await store.waitForCosts()
        #expect(store.costs.isEmpty && store.loading == .loaded)
    }

    @Test("a slow evaluation does not hold the numbers back: the refresh is over and the numbers are there while the cost is still being read")
    func costDoesNotHoldTheNumbersBack() async {
        let store = costStore()
        source.holdEvaluations()
        await store.refresh()
        #expect(store.loading == .loaded && store.orgs.count == 2 && store.costs.isEmpty)
        while source.heldEvaluations < 2 { await Task.yield() }
        source.releaseEvaluations()
        await store.waitForCosts()
        #expect(store.costs.count == 2)
    }

    @Test("an org whose cost is being read is not asked again by another refresh, and is once the read is over and the cost has gone stale")
    func costInFlight() async {
        let clock = Clock()
        let store = costStore(clock)
        source.holdEvaluations()
        await store.refresh()
        await store.refresh()
        await store.refresh()
        while source.heldEvaluations < 2 { await Task.yield() }
        #expect(source.evaluationsAsked.count == 2, "one evaluation for each org, however many refreshes while it is slow")
        source.releaseEvaluations()
        await store.waitForCosts()
        await store.refresh()
        await store.waitForCosts()
        #expect(source.evaluationsAsked.count == 2, "and a cost that was read lately is not asked for again")
        clock.advance(OutcomesStore.costLifetime + 1)
        await store.refresh()
        await store.waitForCosts()
        #expect(source.evaluationsAsked.count == 4)
    }

    @Test("another period cancels the reads under way, what is left of them touches nothing, and the new period's reads keep their markers")
    func costPeriodInFlight() async {
        let store = costStore()
        source.holdEvaluations()
        await store.refresh()
        while source.heldEvaluations < 2 { await Task.yield() }
        await store.choose(days: 30)
        while source.heldEvaluations < 4 { await Task.yield() }
        #expect(source.evaluationsAsked.count == 4)
        source.releaseEvaluations(2)
        for _ in 0..<100 { await Task.yield() }
        #expect(source.cancelledEvaluations == 2, "the reads of the old period were cancelled")
        #expect(store.costs.isEmpty, "what the old period's reads found is not the new period's cost")
        await store.refresh()
        #expect(source.evaluationsAsked.count == 4, "the old reads ending did not clear the new reads' markers")
        source.releaseEvaluations()
        await store.waitForCosts()
        #expect(store.costs.count == 2)
    }
}

