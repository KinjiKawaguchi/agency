@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

enum Fixture {
    static func data(_ name: String, from file: String = #filePath) throws -> Data {
        var url = URL(fileURLWithPath: file)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        return try Data(contentsOf: url.appendingPathComponent("testdata/client/\(name).json"))
    }

    static func decode<T: Decodable>(_ type: T.Type, _ name: String) -> T {
        try! JSONDecoder().decode(T.self, from: data(name))
    }
}

private final class FakeSource: DeskSource, @unchecked Sendable {
    private let lock = NSLock()
    private var _me: Result<Me, Error> = .success(Fixture.decode(Me.self, "me"))
    private var _snapshot: @Sendable (String, String) throws -> TeamSnapshot = { _, _ in Fixture.decode(TeamSnapshot.self, "team-snapshot-local") }
    private var _decisions: Result<DecisionLog, Error> = .success(Fixture.decode(DecisionLog.self, "decision-log"))
    private var _meCalls = 0

    var meCalls: Int { lock.withLock { _meCalls } }
    func set(me: Result<Me, Error>) { lock.withLock { _me = me } }
    func set(snapshot: @escaping @Sendable (String, String) throws -> TeamSnapshot) { lock.withLock { _snapshot = snapshot } }
    func set(decisions: Result<DecisionLog, Error>) { lock.withLock { _decisions = decisions } }

    func me() async throws -> Me {
        try lock.withLock { _meCalls += 1; return try _me.get() }
    }
    func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot {
        let make = lock.withLock { _snapshot }
        return try make(org, team)
    }
    func decisions(limit: Int) async throws -> DecisionLog {
        try lock.withLock { try _decisions.get() }
    }
}

private final class FakeSender: DeskSender, @unchecked Sendable {
    private let lock = NSLock()
    private var _sent: [PreparedDecision] = []
    private var _answer: Result<DecisionResult, Error> = .success(DecisionResult(outcome: .applied, message: nil))
    private var gate: CheckedContinuation<Void, Never>?
    private var holds = false

    var sent: [PreparedDecision] { lock.withLock { _sent } }
    func set(answer: Result<DecisionResult, Error>) { lock.withLock { _answer = answer } }
    func holdNextSend() { lock.withLock { holds = true } }
    func release() { lock.withLock { let waiting = gate; gate = nil; return waiting }?.resume() }
    var isHolding: Bool { lock.withLock { gate != nil } }

    func send(_ decision: PreparedDecision) async throws -> DecisionResult {
        lock.withLock { _sent.append(decision) }
        if lock.withLock({ let held = holds; holds = false; return held }) {
            await withCheckedContinuation { continuation in lock.withLock { gate = continuation } }
        }
        return try lock.withLock { try _answer.get() }
    }
}

private final class MemoryStorage: DraftStorage, @unchecked Sendable {
    private let lock = NSLock()
    private var stored: [String: DeskDraft]
    private var _saves = 0

    init(_ drafts: [String: DeskDraft] = [:]) { stored = drafts }
    var saves: Int { lock.withLock { _saves } }
    var drafts: [String: DeskDraft] { lock.withLock { stored } }
    func load() -> [String: DeskDraft] { lock.withLock { stored } }
    func save(_ drafts: [String: DeskDraft]) { lock.withLock { stored = drafts; _saves += 1 } }
}

private actor Told {
    private(set) var count = 0
    private(set) var readsAtThatTime = 0
    func mark(readsSoFar: Int) { count += 1; readsAtThatTime = readsSoFar }
}

@MainActor
@Suite("DeskStore")
struct DeskStoreTests {
    private let source = FakeSource()
    private let sender = FakeSender()
    private let storage = MemoryStorage()
    private let first = "acme:app:r-1"

    private func makeStore(storage: MemoryStorage? = nil) -> DeskStore {
        DeskStore(source: source, sender: sender, storage: storage ?? self.storage)
    }

    // MARK: Reading

    @Test("refresh reads the teams and the recent decisions, and counts what waits on the owner")
    func refresh() async {
        let store = makeStore()
        #expect(store.loading == .idle)
        await store.refresh()
        #expect(store.loading == .loaded)
        #expect(store.items.map(\.id) == ["acme:app:r-1", "acme:app:r-2"])
        #expect(store.waitingCount == 2)
        #expect(store.recent.count == 3)
        #expect(store.teams.count == 1)
        #expect(!store.needsSignIn)
    }

    @Test("a team that cannot be read does not hide the others")
    func teamFailure() async throws {
        let two = """
        {"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[{"id":"acme","isRoot":true,"label":"Acme","teams":[
        {"id":"app","label":"app","location":"local","viewer":"owner"},{"id":"web","label":"web","location":"remote","viewer":"owner"}]}]}
        """
        source.set(me: .success(try JSONDecoder().decode(Me.self, from: Data(two.utf8))))
        source.set { _, team in
            if team == "web" { throw NodeError.http(status: 503, message: "no replica yet") }
            return Fixture.decode(TeamSnapshot.self, "team-snapshot-local")
        }
        let store = makeStore()
        await store.refresh()
        #expect(store.loading == .loaded)
        #expect(store.items.count == 2)
        #expect(store.teamFailures == ["acme/web": "no replica yet"])
    }

    @Test("a team that was read keeps its last answer when a later read of it fails, and the failure is reported")
    func keepsLastAnswer() async {
        let store = makeStore()
        await store.refresh()
        #expect(store.items.count == 2)
        source.set { _, _ in throw NodeError.http(status: 503, message: "not ready") }
        await store.refresh()
        #expect(store.items.count == 2, "the desk keeps what it showed")
        #expect(store.teamFailures == ["acme/app": "not ready"])
        source.set { _, _ in Fixture.decode(TeamSnapshot.self, "team-snapshot-local") }
        await store.refresh()
        #expect(store.teamFailures.isEmpty)
    }

    @Test("a team the account no longer lists is dropped, not kept")
    func dropsTeamsThatLeft() async throws {
        let store = makeStore()
        await store.refresh()
        let empty = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[]}"#
        source.set(me: .success(try JSONDecoder().decode(Me.self, from: Data(empty.utf8))))
        await store.refresh()
        #expect(store.items.isEmpty)
        #expect(store.teams.isEmpty)
    }

    @Test("a team that left and comes back with a failing read does not bring its old answer back")
    func leftTeamIsForgotten() async throws {
        let store = makeStore()
        await store.refresh()
        let none = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[]}"#
        source.set(me: .success(try JSONDecoder().decode(Me.self, from: Data(none.utf8))))
        await store.refresh()
        source.set(me: .success(Fixture.decode(Me.self, "me")))
        source.set { _, _ in throw NodeError.http(status: 503, message: "not ready") }
        await store.refresh()
        #expect(store.items.isEmpty, "the old answer of a team that left must not come back")
        #expect(store.teamFailures == ["acme/app": "not ready"])
    }

    @Test("after a decision is sent the cache is told, before the desk reads again")
    func tellsTheCache() async {
        let told = Told()
        let store = DeskStore(source: source, sender: sender, storage: storage, afterSend: { await told.mark(readsSoFar: source.meCalls) })
        await store.refresh()
        let before = source.meCalls
        await store.send(.answer("yes"), for: first)
        #expect(await told.count == 1)
        #expect(await told.readsAtThatTime == before, "told before the second read")
        #expect(source.meCalls == before + 1)
    }

    @Test("an ended Access session asks for a new sign-in and keeps what was on the desk")
    func sessionEnded() async {
        let store = makeStore()
        await store.refresh()
        source.set(me: .failure(NodeError.sessionExpired))
        await store.refresh()
        #expect(store.needsSignIn)
        #expect(store.items.count == 2)
        if case .failed = store.loading {} else { Issue.record("loading should have failed") }
        source.set(me: .success(Fixture.decode(Me.self, "me")))
        await store.refresh()
        #expect(!store.needsSignIn)
        #expect(store.loading == .loaded)
    }

    @Test("no Access token yet asks for a sign-in the same way")
    func notSignedIn() async {
        source.set(me: .failure(NodeError.notSignedIn))
        let store = makeStore()
        await store.refresh()
        #expect(store.needsSignIn)
        #expect(store.items.isEmpty)
    }

    @Test("another failure shows the node's reason and keeps the desk")
    func otherFailure() async {
        let store = makeStore()
        await store.refresh()
        source.set(me: .failure(NodeError.http(status: 502, message: nil)))
        await store.refresh()
        #expect(store.loading == .failed("HTTP 502"))
        #expect(!store.needsSignIn)
        #expect(store.items.count == 2)
    }

    @Test("recent decisions that cannot be read are reported without failing the desk")
    func recentFailure() async {
        source.set(decisions: .failure(NodeError.http(status: 500, message: "boom")))
        let store = makeStore()
        await store.refresh()
        #expect(store.loading == .loaded)
        #expect(store.recentFailure == "boom")
        source.set(decisions: .success(Fixture.decode(DecisionLog.self, "decision-log")))
        await store.refresh()
        #expect(store.recentFailure == nil)
        #expect(store.recent.count == 3)
    }

    // MARK: Drafts

    @Test("drafts are kept as they change, and an empty one is dropped")
    func drafts() {
        let store = makeStore()
        store.setText("もう少し待って", for: first)
        store.toggleOption("a", for: first)
        #expect(store.draft(for: first) == DeskDraft(text: "もう少し待って", optionKey: "a"))
        #expect(storage.drafts[first] == DeskDraft(text: "もう少し待って", optionKey: "a"))
        store.toggleOption("a", for: first)
        #expect(store.draft(for: first).optionKey == nil)
        store.setText("", for: first)
        #expect(store.drafts.isEmpty)
        #expect(storage.drafts.isEmpty)
    }

    @Test("drafts come back from the storage at the next launch")
    func draftsReload() {
        let kept = MemoryStorage([first: DeskDraft(text: "x", optionKey: "b")])
        #expect(makeStore(storage: kept).draft(for: first) == DeskDraft(text: "x", optionKey: "b"))
    }

    // MARK: Sending

    @Test("a sent reply goes to the sender as prepared, clears its draft, and the desk is read again")
    func send() async {
        let store = makeStore()
        await store.refresh()
        store.setText("条件つきで", for: first)
        let readsBefore = source.meCalls
        await store.send(.answer("条件つきで"), for: first)
        #expect(sender.sent.count == 1)
        #expect(!sender.sent[0].isComment)
        #expect(sender.sent[0].text == "条件つきで")
        #expect(sender.sent[0].answerWith.target == "r-1")
        #expect(store.draft(for: first).isEmpty)
        #expect(storage.drafts.isEmpty)
        #expect(source.meCalls == readsBefore + 1)
        #expect(store.sending.isEmpty)
    }

    @Test("a sent reply is remembered while the request is still on the desk, and forgotten when it leaves")
    func sentIsRemembered() async {
        let store = makeStore()
        await store.refresh()
        sender.set(answer: .success(DecisionResult(outcome: .queued, message: "node asleep")))
        await store.send(.answer("hm"), for: first)
        #expect(store.sent[first] == SentReply(isComment: false, outcome: .queued, message: "node asleep"))
        source.set { _, _ in
            var snapshot = Fixture.decode(TeamSnapshot.self, "team-snapshot-local")
            snapshot = try! JSONDecoder().decode(TeamSnapshot.self, from: Data(
                String(decoding: try! Fixture.data("team-snapshot-local"), as: UTF8.self)
                    .replacingOccurrences(of: "\"r-1\"", with: "\"r-gone\"").utf8))
            return snapshot
        }
        await store.refresh()
        #expect(store.sent[first] == nil)
    }

    @Test("a comment on a request with options is sent as a comment: no choice, and nothing decided")
    func sendComment() async {
        source.set { _, _ in Teams.snapshot() }
        let store = makeStore()
        await store.refresh()
        let question = "acme:app:" + Teams.questionID
        await store.send(.comment("来週にしてください"), for: question)
        #expect(sender.sent.count == 1)
        #expect(sender.sent[0].isComment)
        #expect(sender.sent[0].choice == nil)
        #expect(store.sent[question]?.isComment == true)
    }

    @Test("a failed send keeps the draft and says why; editing clears the message")
    func sendFails() async {
        let store = makeStore()
        await store.refresh()
        store.setText("keep me", for: first)
        sender.set(answer: .failure(NodeError.http(status: 403, message: "writes are for the owner")))
        await store.send(.answer("keep me"), for: first)
        #expect(store.sendFailures[first] == "writes are for the owner")
        #expect(store.draft(for: first).text == "keep me")
        #expect(store.sent[first] == nil)
        #expect(store.sending.isEmpty)
        store.setText("keep me!", for: first)
        #expect(store.sendFailures[first] == nil)
    }

    @Test("a send that finds the session over asks for a new sign-in and keeps the draft")
    func sendSessionEnded() async {
        let store = makeStore()
        await store.refresh()
        store.setText("x", for: first)
        sender.set(answer: .failure(NodeError.sessionExpired))
        await store.send(.answer("x"), for: first)
        #expect(store.needsSignIn)
        #expect(store.draft(for: first).text == "x")
    }

    @Test("a reply the request does not allow is refused before anything is sent")
    func refusedBeforeSending() async {
        let store = makeStore()
        await store.refresh()
        await store.send(.comment("words"), for: first)
        #expect(sender.sent.isEmpty)
        #expect(store.sendFailures[first] != nil)
        await store.send(.comment("x"), for: "acme:app:unknown")
        #expect(sender.sent.isEmpty)
    }

    @Test("while a reply is being sent, the same item is not sent again")
    func noDoubleSend() async {
        let store = makeStore()
        await store.refresh()
        sender.holdNextSend()
        let running = Task { await store.send(.answer("once"), for: first) }
        while !sender.isHolding { await Task.yield() }
        #expect(store.sending == [first])
        await store.send(.answer("twice"), for: first)
        sender.release()
        await running.value
        #expect(sender.sent.count == 1)
        #expect(sender.sent[0].text == "once")
    }

    @Test("a request that was already sent is not sent again")
    func noResendAfterSent() async {
        let store = makeStore()
        await store.refresh()
        await store.send(.answer("one"), for: first)
        await store.send(.answer("two"), for: first)
        #expect(sender.sent.count == 1)
    }
}
