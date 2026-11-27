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
    private var gate: CheckedContinuation<Void, Never>?
    private var holdsNextMe = false
    private var teamGate: CheckedContinuation<Void, Never>?
    private var heldTeam: String?

    var meCalls: Int { lock.withLock { _meCalls } }
    func set(me: Result<Me, Error>) { lock.withLock { _me = me } }
    func set(snapshot: @escaping @Sendable (String, String) throws -> TeamSnapshot) { lock.withLock { _snapshot = snapshot } }
    func set(decisions: Result<DecisionLog, Error>) { lock.withLock { _decisions = decisions } }

    /// The next `me()` waits until `release()`, so a refresh can be looked at while it is under way.
    func holdNextMe() { lock.withLock { holdsNextMe = true } }
    func release() { lock.withLock { let waiting = gate; gate = nil; return waiting }?.resume() }
    var isHolding: Bool { lock.withLock { gate != nil } }

    func me() async throws -> Me {
        if lock.withLock({ let held = holdsNextMe; holdsNextMe = false; return held }) {
            await withCheckedContinuation { continuation in lock.withLock { gate = continuation } }
        }
        return try lock.withLock { _meCalls += 1; return try _me.get() }
    }
    /// The next read of this team waits until `releaseTeam()`, so the desk can be looked at while it is the last to answer.
    func hold(team: String) { lock.withLock { heldTeam = team } }
    func releaseTeam() { lock.withLock { let waiting = teamGate; teamGate = nil; return waiting }?.resume() }
    var isHoldingTeam: Bool { lock.withLock { teamGate != nil } }

    func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot {
        if lock.withLock({ let held = heldTeam == team; if held { heldTeam = nil }; return held }) {
            await withCheckedContinuation { continuation in lock.withLock { teamGate = continuation } }
        }
        let make = lock.withLock { _snapshot }
        return try make(org, team)
    }
    func decisions(limit: Int) async throws -> DecisionLog {
        try lock.withLock { try _decisions.get() }
    }
}

private final class FakeSigner: DeskSigner, @unchecked Sendable {
    private let lock = NSLock()
    private var _signed: [PreparedDecision] = []
    private var _failure: Error?
    private var gate: CheckedContinuation<Void, Never>?
    private var holds = false
    private var counter = 0

    /// What was asked to be signed.
    var sent: [PreparedDecision] { lock.withLock { _signed } }
    func set(failure: Error?) { lock.withLock { _failure = failure } }
    func holdNextSend() { lock.withLock { holds = true } }
    func release() { lock.withLock { let waiting = gate; gate = nil; return waiting }?.resume() }
    var isHolding: Bool { lock.withLock { gate != nil } }

    func sign(_ decision: PreparedDecision) async throws -> SignedDecision {
        lock.withLock { _signed.append(decision) }
        if lock.withLock({ let held = holds; holds = false; return held }) {
            await withCheckedContinuation { continuation in lock.withLock { gate = continuation } }
        }
        let number = lock.withLock { counter += 1; return counter }
        if let failure = lock.withLock({ _failure }) { throw failure }
        let envelope = decision.envelope(id: "env-\(number)", account: "alice", at: "2026-11-27T10:00:00Z")
        return SignedDecision(envelope: envelope, signer: "d-1", signature: "c2ln", title: decision.title, choiceLabel: decision.choiceLabel)
    }
}

/// What the node answers a kept decision with; or never, while held.
private final class FakeSubmitter: DecisionSubmitter, @unchecked Sendable {
    private let lock = NSLock()
    private var _sent: [SignedDecision] = []
    private var _failure: Error?
    private var gate: CheckedContinuation<Void, Never>?
    private var holds = false

    var sent: [SignedDecision] { lock.withLock { _sent } }
    func set(failure: Error?) { lock.withLock { _failure = failure } }
    func hold() { lock.withLock { holds = true } }
    func release() { lock.withLock { let waiting = gate; gate = nil; return waiting }?.resume() }
    var isHolding: Bool { lock.withLock { gate != nil } }

    func submit(_ decision: SignedDecision, org: String, team: String) async throws -> DecisionResult {
        lock.withLock { _sent.append(decision) }
        if lock.withLock({ holds }) {
            await withCheckedContinuation { continuation in lock.withLock { gate = continuation } }
        }
        if let failure = lock.withLock({ _failure }) { throw failure }
        return try JSONDecoder().decode(DecisionResult.self, from: Data(#"{"outcome":"queued"}"#.utf8))
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

/// Waits until `condition` holds, and fails (instead of hanging for ever) when it does not.
private func waitUntil(_ condition: @Sendable () -> Bool, sourceLocation: SourceLocation = #_sourceLocation) async {
    for _ in 0..<200_000 where !condition() { await Task.yield() }
    #expect(condition(), "waited for a condition that never held", sourceLocation: sourceLocation)
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
    private let sender = FakeSigner()
    private let submitter = FakeSubmitter()
    private let storage = MemoryStorage()
    private let kept = MemoryOutboxStorage()
    private let first = "acme:app:r-1"

    private func makeStore(storage: MemoryStorage? = nil) -> DeskStore {
        DeskStore(
            source: source, signer: sender, outbox: DecisionOutbox(storage: kept, submitter: submitter), storage: storage ?? self.storage, retry: .never
        )
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

    @Test("before the node has answered there is no answer: nothing waits is not claimed, and the count is unknown")
    func beforeAnswer() async {
        let store = makeStore()
        #expect(!store.hasAnswer)
        #expect(store.knownWaitingCount == nil)
        await store.refresh()
        #expect(store.hasAnswer)
        #expect(store.knownWaitingCount == 2)
    }

    @Test("a read that fails before any answer is no answer, and one that fails after an answer keeps it")
    func failedRead() async {
        source.set(me: .failure(NodeError.http(status: 503, message: "down")))
        let store = makeStore()
        await store.refresh()
        #expect(!store.hasAnswer)
        #expect(store.knownWaitingCount == nil)
        source.set(me: .success(Fixture.decode(Me.self, "me")))
        await store.refresh()
        #expect(store.knownWaitingCount == 2)
        source.set(me: .failure(NodeError.http(status: 503, message: "down")))
        await store.refresh()
        #expect(store.loading != .loaded)
        #expect(store.hasAnswer)
        #expect(store.knownWaitingCount == 2)
    }

    @Test("a node that answers with nothing to read has answered: the count is 0, not unknown")
    func answeredWithNothing() async throws {
        let none = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[]}"#
        source.set(me: .success(try JSONDecoder().decode(Me.self, from: Data(none.utf8))))
        let store = makeStore()
        await store.refresh()
        #expect(store.hasAnswer)
        #expect(store.knownWaitingCount == 0)
    }

    @Test("once a read has finished the desk stays answered: an empty desk keeps its 0 while a refresh is under way and after a failed one")
    func answeredStays() async throws {
        let none = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[]}"#
        source.set(me: .success(try JSONDecoder().decode(Me.self, from: Data(none.utf8))))
        let store = makeStore()
        #expect(store.knownWaitingCount == nil)
        await store.refresh()
        #expect(store.knownWaitingCount == 0)
        #expect(store.hasAnswer)

        source.holdNextMe()
        let refreshing = Task { await store.refresh() }
        while !source.isHolding { await Task.yield() }
        #expect(store.loading == .loading)
        #expect(store.hasAnswer)
        #expect(store.knownWaitingCount == 0)
        source.release()
        await refreshing.value

        source.set(me: .failure(NodeError.http(status: 503, message: "down")))
        await store.refresh()
        #expect(store.loading != .loaded)
        #expect(store.hasAnswer)
        #expect(store.knownWaitingCount == 0)
    }

    @Test("a failed read is remembered: it stays set while the next refresh is under way, and a read that finishes whole clears it")
    func readFailing() async throws {
        let store = makeStore()
        #expect(!store.isReadFailing)
        await store.refresh()
        #expect(!store.isReadFailing)

        source.set(me: .failure(NodeError.http(status: 503, message: "down")))
        await store.refresh()
        #expect(store.isReadFailing)

        source.set(me: .success(Fixture.decode(Me.self, "me")))
        source.holdNextMe()
        let refreshing = Task { await store.refresh() }
        while !source.isHolding { await Task.yield() }
        #expect(store.loading == .loading)
        #expect(store.isReadFailing, "a refresh under way is not a read that worked")
        source.release()
        await refreshing.value
        #expect(!store.isReadFailing)
    }

    @Test("a read that was cancelled says nothing about the node: no failure, and what was known stays", arguments: [
        CancellationError() as Error, URLError(.cancelled),
    ])
    func cancelledRead(error: Error) async {
        let store = makeStore()
        await store.refresh()
        #expect(store.loading == .loaded && !store.isReadFailing)
        source.set(me: .failure(error))
        await store.refresh()
        #expect(!store.isReadFailing)
        #expect(store.loading == .loaded, "the desk still shows what it read")
        #expect(store.hasAnswer)
        let first = makeStore()
        source.set(me: .failure(error))
        await first.refresh()
        #expect(first.loading == .idle && !first.isReadFailing, "a first read that was cancelled has not failed either")
    }

    @Test("an expired sign-in and a sign-in that was never made are failed reads too")
    func readFailingForTheSignIn() async {
        let store = makeStore()
        source.set(me: .failure(NodeError.sessionExpired))
        await store.refresh()
        #expect(store.isReadFailing)
        source.set(me: .success(Fixture.decode(Me.self, "me")))
        await store.refresh()
        #expect(!store.isReadFailing)
        source.set(me: .failure(NodeError.notSignedIn))
        await store.refresh()
        #expect(store.isReadFailing)
    }

    @Test("a read that finished with a team that could not be read is a failing read: the number is not the whole desk")
    func partialReadIsFailing() async throws {
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
        #expect(store.isReadFailing)
        source.set { _, _ in Fixture.decode(TeamSnapshot.self, "team-snapshot-local") }
        await store.refresh()
        #expect(!store.isReadFailing)
    }

    @Test("a started recovery the node names is on the desk until the node stops naming it, and a failed read leaves it")
    func recoveries() async throws {
        func me(_ recovery: String) throws -> Me {
            let json = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[{"id":"acme","isRoot":true,"label":"Acme","teams":[]\#(recovery)}]}"#
            return try JSONDecoder().decode(Me.self, from: Data(json.utf8))
        }
        let store = makeStore()
        #expect(store.recoveries.isEmpty)
        source.set(me: .success(try me(#","recovery":{"takesEffectAt":"2026-11-29T09:30:00Z"}"#)))
        await store.refresh()
        #expect(store.recoveries.map(\.orgID) == ["acme"])
        source.set(me: .failure(NodeError.http(status: 503, message: "down")))
        await store.refresh()
        #expect(store.recoveries.map(\.orgID) == ["acme"], "a failed read says nothing of it: it stays")
        source.set(me: .success(try me("")))
        await store.refresh()
        #expect(store.recoveries.isEmpty, "the node no longer says it waits: cancelled or taken")
    }

    @Test("what the node says the viewer is is known once the account has answered: a hosted member, or anyone else, and a later failed read keeps it")
    func standing() async throws {
        func me(_ extra: String) throws -> Me {
            let json = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[]\#(extra)}"#
            return try JSONDecoder().decode(Me.self, from: Data(json.utf8))
        }
        let store = makeStore()
        #expect(store.standing == .loading, "not told yet is not anyone")
        source.set(me: .success(try me(#","guest":{"member":"erin","org":"acme"}"#)))
        await store.refresh()
        #expect(store.standing == .guest(GuestRef(member: "erin", org: "acme")))
        source.set(me: .failure(NodeError.http(status: 503, message: "down")))
        await store.refresh()
        #expect(store.standing.isGuest, "a read that fails later does not make the guest anyone else")
        let owner = makeStore()
        source.set(me: .success(try me("")))
        await owner.refresh()
        #expect(owner.standing == .member)
    }

    @Test("a read that fails with nothing earlier to go by counts as anyone else, and an ended sign-in says nothing of who is asking")
    func standingWhenFailed() async {
        let store = makeStore()
        source.set(me: .failure(NodeError.http(status: 503, message: "down")))
        await store.refresh()
        #expect(store.standing == .unknown)
        let signedOut = makeStore()
        source.set(me: .failure(NodeError.sessionExpired))
        await signedOut.refresh()
        #expect(signedOut.standing == .loading, "the sign-in has to run again before the node can say")
    }

    @Test("what waits for the root key is on the desk until the node stops naming it, a failed read leaves it, and nothing is there before the node answered")
    func rootChanges() async throws {
        func me(_ changes: String) throws -> Me {
            let json = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[{"id":"acme","isRoot":true,"label":"Acme","teams":[]}]\#(changes)}"#
            return try JSONDecoder().decode(Me.self, from: Data(json.utf8))
        }
        let store = makeStore()
        #expect(store.rootChanges == nil)
        source.set(me: .success(try me(#","rootChanges":[{"id":"c1","org":"acme","what":"add member carol (Carol)","receivedAt":"2026-11-26T09:00:00Z"}]"#)))
        await store.refresh()
        #expect(store.rootChanges?.items.map(\.id) == ["c1"])
        source.set(me: .failure(NodeError.http(status: 503, message: "down")))
        await store.refresh()
        #expect(store.rootChanges?.items.map(\.id) == ["c1"], "a failed read says nothing of it: it stays")
        source.set(me: .success(try me("")))
        await store.refresh()
        #expect(store.rootChanges == nil, "the node no longer says it waits: signed or dropped")
    }

    @Test("a read that is cancelled after the account answered keeps the newer list of recoveries")
    func recoveriesKeptWhenCancelledAfterTheAccount() async throws {
        func me(_ recovery: String) throws -> Me {
            let json = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[{"id":"acme","isRoot":true,"label":"Acme","teams":[{"id":"app","label":"app","location":"local","viewer":"owner"}]\#(recovery)}]}"#
            return try JSONDecoder().decode(Me.self, from: Data(json.utf8))
        }
        let store = makeStore()
        source.set(me: .success(try me("")))
        await store.refresh()
        #expect(store.recoveries.isEmpty)
        source.set(me: .success(try me(#","recovery":{}"#)))
        source.hold(team: "app")
        let reading = Task { await store.refresh() }
        while !source.isHoldingTeam { await Task.yield() }
        reading.cancel()
        source.releaseTeam()
        await reading.value
        #expect(store.recoveries.map(\.orgID) == ["acme"], "what the account said stays, whatever became of the rest of the read")
    }

    @Test("during the first read a team that has answered is shown, and the number is still unknown until the read has finished")
    func firstReadPartial() async throws {
        let two = """
        {"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[{"id":"acme","isRoot":true,"label":"Acme","teams":[
        {"id":"app","label":"app","location":"local","viewer":"owner"},{"id":"web","label":"web","location":"local","viewer":"owner"}]}]}
        """
        source.set(me: .success(try JSONDecoder().decode(Me.self, from: Data(two.utf8))))
        source.hold(team: "web")
        let store = makeStore()
        let reading = Task { await store.refresh() }
        // Both teams are read at once: wait until `web` is held and `app` has been shown.
        while !(source.isHoldingTeam && !store.teams.isEmpty) { await Task.yield() }
        #expect(store.hasAnswer)
        #expect(store.knownWaitingCount == nil)
        source.releaseTeam()
        await reading.value
        #expect(store.knownWaitingCount != nil)
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

    @Test("a team whose view is not ready is asked again, and shown once it answers, with nothing reported")
    func asksAgain() async {
        let tries = Counter()
        source.set { _, _ in
            if tries.next() <= 2 { throw NodeError.http(status: 503, message: "the board has not been read yet") }
            return Fixture.decode(TeamSnapshot.self, "team-snapshot-local")
        }
        let store = DeskStore(source: source, signer: sender, outbox: DecisionOutbox(storage: kept, submitter: submitter), storage: storage, retry: RetryPolicy(delays: [1, 1], wait: { _ in }))
        await store.refresh()
        #expect(tries.count == 3)
        #expect(store.items.count == 2)
        #expect(store.teamFailures.isEmpty)
        #expect(store.loading == .loaded)
    }

    @Test("a team that keeps failing is reported after the waits are used up, and keeps its last answer")
    func givesUp() async {
        let store = DeskStore(source: source, signer: sender, outbox: DecisionOutbox(storage: kept, submitter: submitter), storage: storage, retry: RetryPolicy(delays: [1, 1], wait: { _ in }))
        await store.refresh()
        let tries = Counter()
        source.set { _, _ in tries.next(); throw NodeError.http(status: 503, message: "not ready") }
        await store.refresh()
        #expect(tries.count == 3)
        #expect(store.teamFailures == ["acme/app": "not ready"])
        #expect(store.items.count == 2)
    }

    @Test("a team that answers is shown while another is still being asked again")
    func showsWhileWaiting() async throws {
        let two = """
        {"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[{"id":"acme","isRoot":true,"label":"Acme","teams":[
        {"id":"app","label":"app","location":"local","viewer":"owner"},{"id":"web","label":"web","location":"remote","viewer":"owner"}]}]}
        """
        source.set(me: .success(try JSONDecoder().decode(Me.self, from: Data(two.utf8))))
        let webTries = Counter()
        source.set { _, team in
            if team == "web", webTries.next() == 1 { throw NodeError.http(status: 503, message: "not ready") }
            return Fixture.decode(TeamSnapshot.self, "team-snapshot-local")
        }
        let holder = StoreHolder()
        let shownAtTheWait = Counter()
        // The wait before web is asked again: it looks at the desk, giving app time to answer first.
        let policy = RetryPolicy(delays: [1], wait: { _ in
            for _ in 0..<1000 {
                if await MainActor.run(body: { holder.store?.teams.count ?? 0 }) == 1 { break }
                try await Task.sleep(for: .milliseconds(1))
            }
            shownAtTheWait.set(await MainActor.run { holder.store?.teams.count ?? -1 })
        })
        let store = DeskStore(source: source, signer: sender, outbox: DecisionOutbox(storage: kept, submitter: submitter), storage: storage, retry: policy)
        holder.store = store
        await store.refresh()
        #expect(shownAtTheWait.count == 1, "app was on the desk while web was waited for")
        #expect(store.teams.count == 2)
    }

    @Test("an answer that is final is not asked again")
    func finalAnswer() async {
        let tries = Counter()
        source.set { _, _ in tries.next(); throw NodeError.http(status: 403, message: "no") }
        let store = DeskStore(source: source, signer: sender, outbox: DecisionOutbox(storage: kept, submitter: submitter), storage: storage, retry: RetryPolicy(delays: [1, 1], wait: { _ in }))
        await store.refresh()
        #expect(tries.count == 1)
    }

    @Test("after the node has taken a decision the cache is told, before the desk reads again")
    func tellsTheCache() async {
        let told = Told()
        let store = DeskStore(
            source: source, signer: sender, outbox: DecisionOutbox(storage: kept, submitter: submitter), storage: storage,
            afterSend: { await told.mark(readsSoFar: source.meCalls) }, retry: .never
        )
        await store.refresh()
        let before = source.meCalls
        await store.send(.answer("yes"), for: first)
        await store.outbox.flush()
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

    // MARK: Deciding

    @Test("a decision is signed as prepared, kept on the device, and the item leaves the pile and the count at once")
    func decide() async {
        let store = makeStore()
        await store.refresh()
        store.setText("条件つきで", for: first)
        submitter.hold()
        await store.send(.answer("条件つきで"), for: first)
        #expect(sender.sent.count == 1 && !sender.sent[0].isComment && sender.sent[0].text == "条件つきで" && sender.sent[0].answerWith.target == "r-1")
        #expect(store.draft(for: first).isEmpty && storage.drafts.isEmpty)
        #expect(kept.entries.map(\.itemID) == [first], "on the device before anything is sent")
        #expect(!store.items.map(\.id).contains(first), "off the pile")
        #expect(store.waitingCount == 1, "and off the count: the other one still waits")
        #expect(store.sending.isEmpty)
        submitter.release()
    }

    @Test("deciding does not wait for the node: the node never answers and the owner is on the next item")
    func doesNotWait() async {
        let store = makeStore()
        await store.refresh()
        submitter.hold()
        await store.send(.answer("go"), for: first)
        await waitUntil { submitter.isHolding }
        #expect(store.outbox.sending.count == 1, "it is being sent, in the background")
        #expect(store.items.map(\.id) == ["acme:app:r-2"] && store.sending.isEmpty)
        submitter.release()
    }

    @Test("once the node has taken it the entry is gone, and the desk is read again")
    func taken() async {
        let store = makeStore()
        await store.refresh()
        let readsBefore = source.meCalls
        await store.send(.answer("yes"), for: first)
        await store.outbox.flush()
        #expect(store.outbox.entries.isEmpty && kept.entries.isEmpty)
        #expect(submitter.sent.map(\.envelope.id) == ["env-1"])
        #expect(source.meCalls == readsBefore + 1)
    }

    @Test("a decision the node took stays off the pile and the count until the desk has been read again: the request does not flash back")
    func takenStaysOffUntilRead() async {
        let store = makeStore()
        await store.refresh()
        source.holdNextMe()
        await store.send(.answer("yes"), for: first)
        await waitUntil { source.isHolding }
        #expect(store.outbox.entries.isEmpty, "the node took it, and the desk is being read again")
        #expect(!store.items.map(\.id).contains(first) && store.waitingCount == 1)
        source.release()
        for _ in 0..<200_000 where !store.outbox.justTaken.isEmpty { await Task.yield() }
        #expect(store.outbox.justTaken.isEmpty, "and it is let go of once the desk has been read")
    }

    @Test("a decided item does not come back on the pile while its send is pending or refused, though the node's view still lists it, and comes back when discarded")
    func staysOff() async {
        let store = makeStore()
        await store.refresh()
        submitter.set(failure: NodeError.http(status: 409, message: "refused: not yours"))
        await store.send(.answer("hm"), for: first)
        await store.outbox.flush()
        await store.refresh()
        #expect(!store.items.map(\.id).contains(first), "refused for good: it waits for the owner to let go")
        #expect(store.outbox.failed.first?.failure?.reason == "refused: not yours")
        let id = store.outbox.entries[0].id
        store.discardDecision(id)
        #expect(store.items.map(\.id).contains(first) && store.waitingCount == 2)
        #expect(store.draft(for: first) == DeskDraft(text: "hm", optionKey: nil), "what was typed is back in the draft")
    }

    private func log(_ state: String, target: String = "r-1") -> DecisionLog {
        let json = """
        {"entries":[{"choice":"a","decidedAt":"2026-11-27T10:00:00Z","id":"env-1","kind":"answer","org":"acme","state":\(state),
        "target":"\(target)","team":"app","text":""}]}
        """
        return try! JSONDecoder().decode(DecisionLog.self, from: Data(json.utf8))
    }

    @Test("a decision the node took stays off the pile and the count while the decision log says it is queued, though the node's view still lists the request")
    func queuedStaysOff() async {
        let store = makeStore()
        await store.send(.answer("yes"), for: first)
        await store.refresh()
        source.set(decisions: .success(log(#"{"kind":"queued","attempts":1,"nextAt":"2026-11-27T10:00:15Z"}"#)))
        await store.outbox.flush()
        await store.refresh()
        #expect(store.outbox.entries.isEmpty, "the node took it")
        #expect(!store.items.map(\.id).contains(first) && store.waitingCount == 1, "not back before the team has applied it")
        source.set(decisions: .success(log(#"{"kind":"applied"}"#)))
        await store.refresh()
        #expect(store.items.map(\.id).contains(first), "the node's own view decides once the log no longer says queued")
    }

    @Test("a decision the team's node refused later is back on the pile")
    func refusedLaterIsBack() async {
        let store = makeStore()
        await store.refresh()
        await store.send(.answer("yes"), for: first)
        await store.outbox.flush()
        source.set(decisions: .success(log(#"{"kind":"refused","message":"not yours"}"#)))
        await store.refresh()
        #expect(store.items.map(\.id).contains(first))
    }

    @Test("a comment on a request with options is kept as a comment: no choice, and nothing decided")
    func decideComment() async {
        source.set { _, _ in Teams.snapshot() }
        let store = makeStore()
        await store.refresh()
        let question = "acme:app:" + Teams.questionID
        await store.send(.comment("来週にしてください"), for: question)
        #expect(sender.sent.count == 1 && sender.sent[0].isComment && sender.sent[0].choice == nil)
        #expect(store.outbox.entries.first?.isComment == true)
    }

    @Test("a decision that cannot be signed keeps the draft and says why; editing clears the message")
    func signFails() async {
        let store = makeStore()
        await store.refresh()
        store.setText("keep me", for: first)
        sender.set(failure: NodeError.http(status: 403, message: "writes are for the owner"))
        await store.send(.answer("keep me"), for: first)
        #expect(store.sendFailures[first] == "writes are for the owner")
        #expect(store.draft(for: first).text == "keep me")
        #expect(store.outbox.entries.isEmpty && store.items.map(\.id).contains(first))
        #expect(store.sending.isEmpty)
        store.setText("keep me!", for: first)
        #expect(store.sendFailures[first] == nil)
    }

    @Test("a decision that cannot be written to the device is not accepted: the draft stays and the item stays")
    func notKept() async {
        let store = makeStore()
        await store.refresh()
        store.setText("x", for: first)
        kept.failsToWrite = true
        await store.send(.answer("x"), for: first)
        #expect(store.sendFailures[first] == Ja.Outbox.notKept && store.draft(for: first).text == "x")
        #expect(store.outbox.entries.isEmpty && store.items.map(\.id).contains(first) && submitter.sent.isEmpty)
    }

    @Test("a decision that finds the session over asks for a new sign-in and keeps the draft")
    func signSessionEnded() async {
        let store = makeStore()
        await store.refresh()
        store.setText("x", for: first)
        sender.set(failure: NodeError.sessionExpired)
        await store.send(.answer("x"), for: first)
        #expect(store.needsSignIn)
        #expect(store.draft(for: first).text == "x")
    }

    @Test("a reply the request does not allow is refused before anything is signed")
    func refusedBeforeSigning() async {
        let store = makeStore()
        await store.refresh()
        await store.send(.comment("words"), for: first)
        #expect(sender.sent.isEmpty)
        #expect(store.sendFailures[first] != nil)
        await store.send(.comment("x"), for: "acme:app:unknown")
        #expect(sender.sent.isEmpty)
    }

    @Test("while a decision is being signed, the same item is not decided again")
    func noDoubleDecision() async {
        let store = makeStore()
        await store.refresh()
        sender.holdNextSend()
        let running = Task { await store.send(.answer("once"), for: first) }
        await waitUntil { sender.isHolding }
        #expect(store.sending == [first])
        await store.send(.answer("twice"), for: first)
        sender.release()
        await running.value
        #expect(sender.sent.count == 1 && sender.sent[0].text == "once")
    }

    @Test("an item that was already decided is not decided again")
    func noRedecision() async {
        let store = makeStore()
        await store.refresh()
        await store.send(.answer("one"), for: first)
        await store.send(.answer("two"), for: first)
        #expect(sender.sent.count == 1)
    }
}

private final class Counter: @unchecked Sendable {
    private let lock = NSLock()
    private var value = 0

    @discardableResult
    func next() -> Int { lock.withLock { value += 1; return value } }
    func set(_ new: Int) { lock.withLock { value = new } }
    var count: Int { lock.withLock { value } }
}

private final class StoreHolder: @unchecked Sendable {
    @MainActor var store: DeskStore?
}
