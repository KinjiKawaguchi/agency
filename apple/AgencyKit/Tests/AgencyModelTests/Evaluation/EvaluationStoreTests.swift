@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

private final class FakeEvaluation: EvaluationSource, @unchecked Sendable {
    private let lock = NSLock()
    private var meAnswer: Result<Me, Error>
    private var answers: [String: Result<Evaluation, Error>] = [:]
    private var asked: [String] = []
    private var notReady = 0
    private var gates: [CheckedContinuation<Void, Never>] = []
    private var heldDays: Int?

    init(me: Result<Me, Error>) { meAnswer = me }

    var asks: [String] { lock.withLock { asked } }
    func set(me: Result<Me, Error>) { lock.withLock { meAnswer = me } }
    func set(_ answer: Result<Evaluation, Error>, for org: String) { lock.withLock { answers[org] = answer } }
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
