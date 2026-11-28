@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

private final class FakeEvaluation: EvaluationSource, @unchecked Sendable {
    private let lock = NSLock()
    private var meAnswer: Result<Me, Error>
    private var answers: [String: Result<Evaluation, Error>] = [:]
    private var efforts: [String: Result<Effort, Error>] = [:]
    private var planAnswer: Result<OrgPlans, Error> = .success(OrgPlans(byTeam: [:]))
    private var asked: [String] = []
    private var effortAsked: [String] = []
    private var notReady = 0
    private var gates: [CheckedContinuation<Void, Never>] = []
    private var heldDays: Int?

    init(me: Result<Me, Error>) { meAnswer = me }

    var asks: [String] { lock.withLock { asked } }
    var effortAsks: [String] { lock.withLock { effortAsked } }
    func set(me: Result<Me, Error>) { lock.withLock { meAnswer = me } }
    func set(_ answer: Result<Evaluation, Error>, for org: String) { lock.withLock { answers[org] = answer } }
    func set(effort: Result<Effort, Error>, for org: String) { lock.withLock { efforts[org] = effort } }
    func set(plans: Result<OrgPlans, Error>) { lock.withLock { planAnswer = plans } }
    func failWithNotReady(times: Int) { lock.withLock { notReady = times } }

    /// Holds the reads for this period until `release()`, so another period can be chosen meanwhile.
    func hold(days: Int) { lock.withLock { heldDays = days } }
    func release() { lock.withLock { let waiting = gates; gates = []; heldDays = nil; return waiting }.forEach { $0.resume() } }
    var holding: Int { lock.withLock { gates.count } }

    func me() async throws -> Me { try lock.withLock { try meAnswer.get() } }

    func evaluation(org: String, days: Int) async throws -> Evaluation {
        // The answer is the one standing when the read starts; a held read delivers it late.
        let answer: Result<Evaluation, Error> = lock.withLock {
            asked.append("\(org)?days=\(days)")
            if notReady > 0 {
                notReady -= 1
                return .failure(NodeError.http(status: 503, message: "the team's node could not be reached"))
            }
            return answers[org] ?? Result { try TeamEvaluationReportTests.sample() }
        }
        if lock.withLock({ heldDays == days }) {
            await withCheckedContinuation { continuation in lock.withLock { gates.append(continuation) } }
        }
        return try answer.get()
    }

    func effort(org: String, days: Int) async throws -> Effort {
        try lock.withLock {
            effortAsked.append("\(org)?days=\(days)")
            return try (efforts[org] ?? Result { try EffortReportTests.sample() }).get()
        }
    }

    func orgPlans(org: String) async throws -> OrgPlans {
        try lock.withLock { try planAnswer.get() }
    }

    private var aboutAnswer: Result<About, Error> = .success(About(capabilities: []))
    private var budgetAnswers: [String: Result<BudgetStatus, Error>] = [:]
    private var budgetAsked: [String] = []
    func set(about: Result<About, Error>) { lock.withLock { aboutAnswer = about } }
    func set(budget: Result<BudgetStatus, Error>, for team: String) { lock.withLock { budgetAnswers[team] = budget } }
    var budgetAsks: [String] { lock.withLock { budgetAsked } }

    private var aboutCount = 0
    private var budgetHeld = false
    private var budgetArrivals = 0
    private var budgetGates: [Int: CheckedContinuation<Void, Never>] = [:]
    var aboutAsks: Int { lock.withLock { aboutCount } }
    /// Holds the budget reads until they are released.
    func holdBudgets() { lock.withLock { budgetHeld = true } }
    /// Lets every held read answer, and no later one wait.
    func releaseBudgets() { lock.withLock { let waiting = budgetGates; budgetGates = [:]; budgetHeld = false; return waiting }.values.forEach { $0.resume() } }
    /// Lets one held read answer, by the order the reads arrived in (0 is the first).
    func releaseBudgetRead(_ number: Int) { lock.withLock { budgetGates.removeValue(forKey: number) }?.resume() }
    var heldBudgetReads: Int { lock.withLock { budgetGates.count } }
    /// For a team that is not asked for in another org: its answer in `org`.
    func set(budget: Result<BudgetStatus, Error>, for team: String, in org: String) { lock.withLock { budgetAnswers["\(org)/\(team)"] = budget } }

    func about() async throws -> About {
        try lock.withLock {
            aboutCount += 1
            return try aboutAnswer.get()
        }
    }

    func budget(org: String, team: String) async throws -> BudgetStatus {
        // The answer is the one standing when the read starts; a held read delivers it late.
        let (answer, number): (Result<BudgetStatus, Error>, Int?) = lock.withLock {
            budgetAsked.append("\(org)/\(team)")
            let answer = budgetAnswers["\(org)/\(team)"] ?? budgetAnswers[team] ?? .failure(NodeError.http(status: 404, message: nil))
            guard budgetHeld else { return (answer, nil) }
            budgetArrivals += 1
            return (answer, budgetArrivals - 1)
        }
        if let number {
            await withCheckedContinuation { continuation in lock.withLock { budgetGates[number] = continuation } }
        }
        return try answer.get()
    }
}

private func me(_ json: String) -> Me { try! JSONDecoder().decode(Me.self, from: Data(json.utf8)) }

private let account = #""account":{"id":"alice","label":"ALICE","fingerprint":"f"},"funding":[]"#
private let twoOrgs = me(#"{\#(account),"orgs":[{"id":"acme","isRoot":true,"label":"Acme","teams":[{"id":"app","label":"The app","location":"local","viewer":"owner"},{"id":"docs","label":"docs","location":"local","viewer":"owner"}]},{"id":"zed","isRoot":false,"label":"Zed","teams":[]}]}"#)

@MainActor
@Suite("EvaluationStore")
struct EvaluationStoreTests {
    @Test("reads the first org over 14 days, and names each team by the label the account gives it")
    func reads() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        #expect(store.loading == .loaded)
        #expect(store.org?.id == "acme")
        #expect(store.days == 14)
        #expect(source.asks == ["acme?days=14"])
        guard case .ready(let reports, let unavailable) = store.state else { Issue.record("not ready: \(store.state)"); return }
        #expect(reports.map(\.title) == ["The appチーム", "docsチーム"])
        #expect(unavailable.map(\.title) == ["opsチーム"])
        #expect(unavailable.map(\.text) == ["このチームは数字を返せませんでした: the team's node could not be reached"])
    }

    @Test("the effort and the org's plans are read with the evaluation, for the same org and period")
    func readsEffort() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        await store.choose(days: 30)
        #expect(source.effortAsks == ["acme?days=14", "acme?days=30"])
        guard case .ready(let reports, _) = store.effort else { Issue.record("not ready: \(store.effort)"); return }
        #expect(reports.map(\.title) == ["The appチーム"])
        #expect(reports[0].planNote != nil)
    }

    @Test("a team's plan from the org definition is compared, and an org definition that cannot be read says so")
    func plans() async throws {
        let source = FakeEvaluation(me: .success(twoOrgs))
        let plan = TeamPlan(allocation: [.newFeature: 50, .improvement: 20, .maintenance: 20, .debt: 10, .incident: 0], weeklyBudgetUsd: 140)
        source.set(plans: .success(OrgPlans(byTeam: ["app": plan])))
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        guard case .ready(let stated, _) = store.effort else { Issue.record("not ready"); return }
        #expect(stated[0].planNote == nil)
        #expect(stated[0].plannedBudget == 140)

        source.set(plans: .failure(NodeError.http(status: 500, message: "boom")))
        await store.refresh()
        guard case .ready(let unreadable, _) = store.effort else { Issue.record("not ready"); return }
        #expect(unreadable[0].planNote == Ja.Effort.planUnreadable)
    }

    // MARK: The weekly budget

    private func status(_ part: BudgetPart, org: BudgetPart = BudgetPart(capUsd: 500, spentUsd: 10, state: .ok)) -> BudgetStatus {
        BudgetStatus(weekStart: "2026-11-23", team: part, org: org)
    }

    @Test("a node that lists the capability is asked for each team of the effort, and the answer is kept by team")
    func budgetAsked() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        source.set(about: .success(About(capabilities: ["budget"])))
        source.set(budget: .success(status(BudgetPart(capUsd: 200, spentUsd: 163.2, state: .warning))), for: "app")
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        await store.budgetsSettled()
        #expect(source.budgetAsks == ["acme/app"])
        guard case .reported(let report)? = store.budgets["app"] else { Issue.record("no report: \(String(describing: store.budgets["app"]))"); return }
        #expect(report.meter?.state == .warning)
        #expect(report.line == "今週(2026-11-23 の月曜から)は $200 のうち $163")
    }

    @Test("a node that does not list the capability is not asked, and the plan's budget is said as what the plan states")
    func budgetNotListed() async {
        for about: Result<About, Error> in [.success(About(capabilities: ["webPush"])), .failure(NodeError.http(status: 404, message: nil))] {
            let source = FakeEvaluation(me: .success(twoOrgs))
            source.set(about: about)
            source.set(budget: .success(status(BudgetPart(capUsd: 1, spentUsd: 1, state: .over))), for: "app")
            source.set(plans: .success(OrgPlans(byTeam: ["app": TeamPlan(allocation: [.newFeature: 100, .improvement: 0, .maintenance: 0, .debt: 0, .incident: 0], weeklyBudgetUsd: 140)])))
            let store = EvaluationStore(source: source, retry: .never)
            await store.refresh()
            await store.budgetsSettled()
        await store.budgetsSettled()
            #expect(source.budgetAsks.isEmpty, "not asked")
            #expect(store.budgets["app"] == .plain("週の予算は $140 です。このノードは、今週の使用額をまだ返しません。"))
        }
    }

    @Test("a team with no plan, on a node that does not report the week, has nothing to say")
    func budgetNothing() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        await store.budgetsSettled()
        #expect(store.budgets["app"] == BudgetRead.none)
    }

    private func settle() async {
        for _ in 0..<100 { await Task.yield() }
    }

    @Test("a budget read of the org that was shown does not land on the org chosen after it, whenever it answers")
    func oldOrgBudgetsDoNotLand() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        source.set(about: .success(About(capabilities: ["budget"])))
        let acme = status(BudgetPart(capUsd: 200, spentUsd: 10, state: .ok))
        let zed = status(BudgetPart(capUsd: 100, spentUsd: 260, state: .over))
        source.set(budget: .success(acme), for: "app", in: "acme")
        source.set(budget: .success(zed), for: "app", in: "zed")
        source.holdBudgets()
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        while source.heldBudgetReads < 1 { await Task.yield() }
        await store.choose(org: "zed")
        while source.heldBudgetReads < 2 { await Task.yield() }
        #expect(store.budgets["app"] == .loading, "the new org says it is loading, and nothing of the old one")
        // The new org's read answers first, and then the old one's, late.
        source.releaseBudgetRead(1)
        await store.budgetsSettled()
        #expect(store.budgets["app"] == .reported(BudgetReport(zed)))
        source.releaseBudgetRead(0)
        await settle()
        #expect(store.budgets["app"] == .reported(BudgetReport(zed)), "the old org's answer did not overwrite the new one's")
    }

    @Test("a budget read of the org that was shown, answering before the new org's, leaves the new org loading and not answered with the old")
    func oldOrgAnswersFirst() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        source.set(about: .success(About(capabilities: ["budget"])))
        let acme = status(BudgetPart(capUsd: 200, spentUsd: 10, state: .ok))
        let zed = status(BudgetPart(capUsd: 100, spentUsd: 260, state: .over))
        source.set(budget: .success(acme), for: "app", in: "acme")
        source.set(budget: .success(zed), for: "app", in: "zed")
        source.holdBudgets()
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        while source.heldBudgetReads < 1 { await Task.yield() }
        await store.choose(org: "zed")
        while source.heldBudgetReads < 2 { await Task.yield() }
        source.releaseBudgetRead(0)
        await settle()
        #expect(store.budgets["app"] == .loading, "the old org's answer is not the new org's")
        source.releaseBudgetRead(1)
        await store.budgetsSettled()
        #expect(store.budgets["app"] == .reported(BudgetReport(zed)))
    }

    @Test("an older read of the same org does not overwrite what a newer read said")
    func olderReadDoesNotOverwrite() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        source.set(about: .success(About(capabilities: ["budget"])))
        let before = status(BudgetPart(capUsd: 200, spentUsd: 10, state: .ok))
        let after = status(BudgetPart(capUsd: 200, spentUsd: 170, state: .warning))
        source.set(budget: .success(before), for: "app")
        source.holdBudgets()
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        while source.heldBudgetReads < 1 { await Task.yield() }
        source.set(budget: .success(after), for: "app")
        await store.refresh()
        while source.heldBudgetReads < 2 { await Task.yield() }
        source.releaseBudgetRead(1)
        await store.budgetsSettled()
        #expect(store.budgets["app"] == .reported(BudgetReport(after)))
        source.releaseBudgetRead(0)
        await settle()
        #expect(store.budgets["app"] == .reported(BudgetReport(after)), "the read that began earlier did not overwrite the later one")
    }

    @Test("a budget read of an org that is gone from the account does not land while the whole screen is read again, before the new read has begun")
    func orgGoneBeforeTheNewRead() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        source.set(about: .success(About(capabilities: ["budget"])))
        source.set(budget: .success(status(BudgetPart(capUsd: 200, spentUsd: 10, state: .ok))), for: "app", in: "acme")
        source.holdBudgets()
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        while source.heldBudgetReads < 1 { await Task.yield() }
        #expect(store.budgets["app"] == .loading)
        // The account no longer lists the org that was shown, and the read of the whole screen is under way, not yet at its end.
        source.set(me: .success(me(#"{\#(account),"orgs":[{"id":"zed","isRoot":false,"label":"Zed","teams":[{"id":"app","label":"The app","location":"local","viewer":"owner"}]}]}"#)))
        source.hold(days: 14)
        let again = Task { await store.refresh() }
        while source.holding < 1 { await Task.yield() }
        #expect(store.org?.id == "zed")
        source.releaseBudgetRead(0)
        await settle()
        #expect(store.budgets["app"] == .loading, "the answer for the org that is gone is not shown for the one that replaced it")
        source.release()
        source.releaseBudgets()
        await again.value
        await store.budgetsSettled()
    }

    @Test("a 404 for a team on a node that lists the capability says what the plan states; any other failure is said, with its reason")
    func budgetFailures() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        source.set(about: .success(About(capabilities: ["budget"])))
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        await store.budgetsSettled()
        #expect(store.budgets["app"] == BudgetRead.none, "the team's route answered 404")
        source.set(budget: .failure(NodeError.http(status: 500, message: "boom")), for: "app")
        await store.refresh()
        await store.budgetsSettled()
        #expect(store.budgets["app"] == .failed("boom"))
        let later = FakeEvaluation(me: .success(twoOrgs))
        later.set(about: .failure(NodeError.http(status: 500, message: "no about")))
        let unsaid = EvaluationStore(source: later, retry: .never)
        await unsaid.refresh()
        await unsaid.budgetsSettled()
        #expect(unsaid.budgets["app"] == .failed("no about"))
    }

    @Test("what was said of a team's week stays on screen when a later read fails")
    func budgetStays() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        source.set(about: .success(About(capabilities: ["budget"])))
        source.set(budget: .success(status(BudgetPart(capUsd: 200, spentUsd: 10, state: .ok))), for: "app")
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        await store.budgetsSettled()
        let before = store.budgets["app"]
        source.set(budget: .failure(NodeError.http(status: 500, message: "boom")), for: "app")
        await store.refresh()
        await store.budgetsSettled()
        #expect(store.budgets["app"] == before)
    }

    @Test("choosing another org drops the budgets of the one that was shown")
    func budgetsOfAnotherOrg() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        source.set(about: .success(About(capabilities: ["budget"])))
        source.set(budget: .success(status(BudgetPart(capUsd: 200, spentUsd: 10, state: .ok))), for: "app")
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        await store.budgetsSettled()
        #expect(store.budgets["app"] != nil)
        source.set(effort: .failure(NodeError.http(status: 404, message: nil)), for: "zed")
        await store.choose(org: "zed")
        await store.budgetsSettled()
        #expect(store.budgets.isEmpty)
    }

    @Test("the screen is loaded when the effort is on it, and a team that is slow to answer about its budget does not hold it")
    func budgetsDoNotHoldTheScreen() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        source.set(about: .success(About(capabilities: ["budget"])))
        source.set(budget: .success(status(BudgetPart(capUsd: 200, spentUsd: 10, state: .ok))), for: "app")
        source.holdBudgets()
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        #expect(store.loading == .loaded)
        guard case .ready = store.effort else { Issue.record("the effort is not on screen"); return }
        #expect(store.budgets["app"] == .loading, "the team says it is loading while the node answers")
        while source.heldBudgetReads == 0 { await Task.yield() }
        source.releaseBudgets()
        await store.budgetsSettled()
        guard case .reported? = store.budgets["app"] else { Issue.record("no report: \(String(describing: store.budgets["app"]))"); return }
    }

    @Test("the budget is the node's current week: a period chosen does not ask for it again, and what the node said of itself is asked once")
    func budgetsAreNotAskedForAPeriod() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        source.set(about: .success(About(capabilities: ["budget"])))
        source.set(budget: .success(status(BudgetPart(capUsd: 200, spentUsd: 10, state: .ok))), for: "app")
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        await store.budgetsSettled()
        await store.choose(days: 30)
        await store.choose(days: 90)
        await store.budgetsSettled()
        #expect(source.budgetAsks == ["acme/app"], "asked once, not for each period")
        await store.refresh()
        await store.budgetsSettled()
        #expect(source.budgetAsks == ["acme/app", "acme/app"], "a read of the whole screen asks again")
        #expect(source.aboutAsks == 1, "the node's capabilities are asked once")
    }

    @Test("a node that could not say what it is is asked again at the next read, and a node without the route is not")
    func aboutIsKeptOnlyOnceAnswered() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        source.set(about: .failure(NodeError.http(status: 500, message: "boom")))
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        await store.budgetsSettled()
        await store.refresh()
        await store.budgetsSettled()
        #expect(source.aboutAsks == 2)
        source.set(about: .failure(NodeError.http(status: 404, message: nil)))
        await store.refresh()
        await store.budgetsSettled()
        await store.refresh()
        await store.budgetsSettled()
        #expect(source.aboutAsks == 3, "a 404 is an answer: no capabilities, once and for all")
    }

    @Test("404 on the effort route means not served yet, and 503 that the team could not answer; the evaluation is not affected")
    func effortOutcomes() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        source.set(effort: .failure(NodeError.http(status: 404, message: nil)), for: "acme")
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        #expect(store.effort == .unavailable)
        guard case .ready = store.state else { Issue.record("the evaluation was affected"); return }
        source.set(effort: .failure(NodeError.http(status: 503, message: "runtime busy")), for: "acme")
        await store.refresh()
        #expect(store.effort == .couldNotAnswer("runtime busy"))
    }

    @Test("effort on screen stays when a refresh keeps failing")
    func keepsEffort() async throws {
        let source = FakeEvaluation(me: .success(twoOrgs))
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        source.set(effort: .failure(NodeError.http(status: 503, message: "runtime busy")), for: "acme")
        await store.refresh()
        guard case .ready = store.effort else { Issue.record("the effort was replaced: \(store.effort)"); return }
    }

    @Test("choosing another period or org reads again for it, and only a period on offer")
    func chooses() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        await store.choose(days: 90)
        await store.choose(org: "zed")
        await store.choose(days: 5)
        await store.choose(days: 90)
        await store.choose(org: "nowhere")
        #expect(source.asks == ["acme?days=14", "acme?days=90", "zed?days=90"])
        #expect(store.org?.id == "zed")
        #expect(store.days == 90)
    }

    @Test("an org that stays chosen across a refresh, and one that left falls back to the first")
    func keepsOrg() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        await store.choose(org: "zed")
        await store.refresh()
        #expect(store.org?.id == "zed")
        source.set(me: .success(me(#"{\#(account),"orgs":[{"id":"acme","isRoot":true,"label":"Acme","teams":[]}]}"#)))
        await store.refresh()
        #expect(store.org?.id == "acme")
    }

    @Test("404 means the node does not serve it yet, 503 that the team could not answer, anything else a failure")
    func outcomes() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        source.set(.failure(NodeError.http(status: 404, message: nil)), for: "acme")
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        #expect(store.state == .unavailable)
        source.set(.failure(NodeError.http(status: 503, message: "runtime busy")), for: "acme")
        await store.refresh()
        #expect(store.state == .couldNotAnswer("runtime busy"))
        source.set(.failure(NodeError.http(status: 500, message: "boom")), for: "acme")
        await store.refresh()
        #expect(store.state == .failed("boom"))
        source.set(.failure(NodeError.unreadable("bad shape")), for: "acme")
        await store.refresh()
        #expect(store.state == .couldNotAnswer(Ja.Evaluation.unreadable))
    }

    @Test("an answer that is not ready is asked again, and is shown as a failure only when it keeps failing")
    func notReady() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        source.failWithNotReady(times: 2)
        let store = EvaluationStore(source: source, retry: RetryPolicy(delays: [1, 2, 4], wait: { _ in }))
        await store.refresh()
        #expect(source.asks.count == 3)
        guard case .ready = store.state else { Issue.record("not ready: \(store.state)"); return }

        let stuck = FakeEvaluation(me: .success(twoOrgs))
        stuck.failWithNotReady(times: 100)
        let second = EvaluationStore(source: stuck, retry: RetryPolicy(delays: [1], wait: { _ in }))
        await second.refresh()
        #expect(second.state == .couldNotAnswer("the team's node could not be reached"))
    }

    @Test("numbers on screen stay when a refresh keeps failing, and a final answer replaces them")
    func keepsNumbers() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        let store = EvaluationStore(source: source, retry: .never)
        await store.refresh()
        source.set(.failure(NodeError.http(status: 503, message: "runtime busy")), for: "acme")
        await store.refresh()
        guard case .ready = store.state else { Issue.record("the numbers were replaced: \(store.state)"); return }
        source.set(.failure(NodeError.http(status: 404, message: nil)), for: "acme")
        await store.refresh()
        #expect(store.state == .unavailable)
    }

    @Test("an answer for a period that is no longer the chosen one does not overwrite the new period's")
    func latePeriod() async {
        let source = FakeEvaluation(me: .success(twoOrgs))
        let store = EvaluationStore(source: source, retry: .never)
        source.hold(days: 14)
        let late = Task { await store.refresh() }
        while source.holding < 1 { await Task.yield() }
        source.set(.failure(NodeError.http(status: 404, message: nil)), for: "acme")
        await store.choose(days: 30)
        #expect(store.state == .unavailable)
        source.release()
        await late.value
        #expect(store.days == 30)
        #expect(store.state == .unavailable)
    }

    @Test("an ended sign-in is a sign-in to run again; an account with no org has nothing to read")
    func signInAndNoOrgs() async {
        let ended = EvaluationStore(source: FakeEvaluation(me: .failure(NodeError.sessionExpired)), retry: .never)
        await ended.refresh()
        #expect(ended.needsSignIn)
        #expect(ended.loading == .failed(Ja.Desk.signInEnded))

        let source = FakeEvaluation(me: .success(me(#"{\#(account),"orgs":[]}"#)))
        let none = EvaluationStore(source: source, retry: .never)
        await none.refresh()
        #expect(none.org == nil)
        #expect(source.asks.isEmpty)
    }
}
