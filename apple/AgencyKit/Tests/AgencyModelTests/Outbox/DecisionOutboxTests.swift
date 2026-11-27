import AgencyAPI
import AgencyProtocol
@testable import AgencyModel
import Foundation
import Testing

private final class ScriptedSubmitter: DecisionSubmitter, @unchecked Sendable {
    private let lock = NSLock()
    private var answers: [Result<DecisionResult, Error>]
    private var seen: [SignedDecision] = []

    init(_ answers: [Result<DecisionResult, Error>]) { self.answers = answers }

    var sent: [SignedDecision] { lock.withLock { seen } }

    func submit(_ decision: SignedDecision, org: String, team: String) async throws -> DecisionResult {
        try lock.withLock {
            seen.append(decision)
            guard !answers.isEmpty else { throw NodeError.http(status: 599, message: "no answer scripted") }
            return try answers.removeFirst().get()
        }
    }
}

/// Waits as long as it is allowed to, so that the send can be cut short.
private final class SlowSubmitter: DecisionSubmitter, @unchecked Sendable {
    func submit(_ decision: SignedDecision, org: String, team: String) async throws -> DecisionResult {
        try await Task.sleep(for: .seconds(600))
        throw NodeError.http(status: 599, message: "not reached")
    }
}

/// A read of the desk that waits until `release()`.
private final class HeldRead: @unchecked Sendable {
    private let lock = NSLock()
    private var continuation: CheckedContinuation<Void, Never>?
    private var isReleased = false

    var isWaiting: Bool { lock.withLock { continuation != nil } }

    func release() {
        let waiting = lock.withLock { () -> CheckedContinuation<Void, Never>? in
            isReleased = true
            defer { continuation = nil }
            return continuation
        }
        waiting?.resume()
    }

    func wait() async {
        await withCheckedContinuation { next in
            let passes = lock.withLock { () -> Bool in
                if isReleased { return true }
                continuation = next
                return false
            }
            if passes { next.resume() }
        }
    }
}

/// Holds every send until `open()`.
private final class GateSubmitter: DecisionSubmitter, @unchecked Sendable {
    private let lock = NSLock()
    private var waiting: [CheckedContinuation<Void, Never>] = []
    private var isOpen = false
    private var seen: [String] = []

    var sent: [String] { lock.withLock { seen } }
    var isHolding: Bool { lock.withLock { !waiting.isEmpty } }

    func open() {
        let held = lock.withLock { () -> [CheckedContinuation<Void, Never>] in
            isOpen = true
            defer { waiting = [] }
            return waiting
        }
        for continuation in held { continuation.resume() }
    }

    func submit(_ decision: SignedDecision, org: String, team: String) async throws -> DecisionResult {
        await withCheckedContinuation { continuation in
            let passes = lock.withLock { () -> Bool in
                if isOpen { return true }
                waiting.append(continuation)
                return false
            }
            if passes { continuation.resume() }
        }
        lock.withLock { seen.append(decision.envelope.id) }
        return try took.get()
    }
}

private func answer(_ outcome: String) -> Result<DecisionResult, Error> {
    .success(try! JSONDecoder().decode(DecisionResult.self, from: Data(#"{"outcome":"\#(outcome)"}"#.utf8)))
}

private let took = answer("queued")
private func failing(_ error: Error) -> Result<DecisionResult, Error> { .failure(error) }

private final class Clock: @unchecked Sendable {
    private let lock = NSLock()
    private var current = Date(timeIntervalSince1970: 1_800_000_000)
    var now: Date { lock.withLock { current } }
    func advance(_ seconds: TimeInterval) { lock.withLock { current = current.addingTimeInterval(seconds) } }
}

private func pending(_ number: Int, item: String? = nil, at date: Date) -> PendingDecision {
    let envelope = DecisionEnvelope(
        id: "id-\(number)", kind: .answer, org: "acme", team: "app", target: "r-\(number)", text: "ok", choice: "a",
        account: "alice", at: "2026-11-27T10:00:00Z"
    )
    let signed = SignedDecision(envelope: envelope, signer: "d-1", signature: "c2ln", title: "Title \(number)", choiceLabel: "Yes")
    return PendingDecision(itemID: item ?? "acme:app:r-\(number)", signed: signed, isComment: false, createdAt: date)
}

@MainActor
@Suite("Background sending")
struct BackgroundSendingTests {
    private let clock = Clock()

    private func kept(_ numbers: Int...) throws -> MemoryOutboxStorage {
        let storage = MemoryOutboxStorage()
        try storage.save(numbers.map { pending($0, at: clock.now) })
        return storage
    }

    private func run(_ storage: MemoryOutboxStorage, _ answers: [Result<DecisionResult, Error>]) async -> (BackgroundSending.Result, ScriptedSubmitter) {
        let submitter = ScriptedSubmitter(answers)
        let clock = clock
        return (await BackgroundSending.run(storage: storage, submitter: submitter, now: { clock.now }), submitter)
    }

    @Test("a woken app sends what is kept from the device's file, with no key, and what the node took leaves it")
    func sends() async throws {
        let storage = try kept(1, 2)
        let (result, submitter) = await run(storage, [took, took])
        #expect(submitter.sent.map(\.envelope.id) == ["id-1", "id-2"])
        #expect(storage.entries.isEmpty && result.decided.isEmpty && result.retryAt == nil)
    }

    @Test("what could not be sent is kept, its item stays decided, and the result says when to try again")
    func retries() async throws {
        let storage = try kept(1, 2)
        let (result, _) = await run(storage, [failing(URLError(.notConnectedToInternet)), took])
        #expect(storage.entries.map(\.id) == ["id-1"])
        #expect(result.decided == ["acme:app:r-1"])
        #expect(result.retryAt == clock.now.addingTimeInterval(5))
    }

    @Test("nothing is tried again for what only the owner can mend: a decision that stopped, or a sign-in that ended")
    func waitsForTheOwner() async throws {
        let stopped = try kept(1)
        let (first, _) = await run(stopped, [failing(NodeError.http(status: 409, message: "refused"))])
        #expect(first.decided == ["acme:app:r-1"] && first.retryAt == nil)
        let signedOut = try kept(2)
        let (second, _) = await run(signedOut, [failing(NodeError.sessionExpired)])
        #expect(second.decided == ["acme:app:r-2"] && second.retryAt == nil && signedOut.entries.count == 1)
    }

    @Test("an entry that is not due yet is left alone")
    func notDue() async throws {
        let storage = try kept(1)
        _ = await run(storage, [failing(URLError(.timedOut))])
        let (result, submitter) = await run(storage, [took])
        #expect(submitter.sent.isEmpty, "five seconds have not passed")
        #expect(result.decided == ["acme:app:r-1"] && result.retryAt == clock.now.addingTimeInterval(5))
    }

    @Test("a storage that cannot be read sends nothing and keeps what is in the file")
    func unreadable() async throws {
        let storage = try kept(1)
        storage.failsToRead = true
        let (result, submitter) = await run(storage, [took])
        #expect(submitter.sent.isEmpty && result.decided.isEmpty && result.retryAt == nil)
        storage.failsToRead = false
        #expect(storage.entries.map(\.id) == ["id-1"])
    }
}

@MainActor
@Suite("Decision outbox")
struct DecisionOutboxTests {
    private let clock = Clock()

    private func decision(_ number: Int, item: String? = nil) -> PendingDecision {
        pending(number, item: item, at: clock.now)
    }

    private func outbox(_ answers: [Result<DecisionResult, Error>], storage: MemoryOutboxStorage = MemoryOutboxStorage()) -> (DecisionOutbox, ScriptedSubmitter, MemoryOutboxStorage) {
        let submitter = ScriptedSubmitter(answers)
        let clock = clock
        return (DecisionOutbox(storage: storage, submitter: submitter, now: { clock.now }), submitter, storage)
    }

    @Test("a decision is written to the device before keep returns, and a decision that cannot be written is not accepted")
    func keeping() throws {
        let (outbox, _, storage) = outbox([])
        try outbox.keep(decision(1))
        #expect(storage.entries.map(\.id) == ["id-1"], "on the device already")
        storage.failsToWrite = true
        #expect(throws: OutboxError.notKept) { try outbox.keep(decision(2)) }
        #expect(outbox.entries.map(\.id) == ["id-1"], "not accepted: nothing is promised that a kill could lose")
    }

    @Test("kept decisions that cannot be read now are not replaced: nothing new is kept until they can be read, and then they are all still there")
    func unreadable() throws {
        let storage = MemoryOutboxStorage([decision(1)])
        storage.failsToRead = true
        let (outbox, _, _) = outbox([], storage: storage)
        #expect(!outbox.isReadable)
        #expect(throws: OutboxError.notKept) { try outbox.keep(decision(2)) }
        #expect(storage.entries.map(\.id) == ["id-1"], "the file was not written over")
        storage.failsToRead = false
        try outbox.keep(decision(2))
        #expect(outbox.isReadable && storage.entries.map(\.id) == ["id-1", "id-2"], "what was kept is there, and the new one with it")
    }

    @Test("kept decisions are not read again in the middle of a send: the send has its own view of them")
    func noReloadWhileSending() async throws {
        let gate = GateSubmitter()
        let storage = MemoryOutboxStorage()
        let outbox = DecisionOutbox(storage: storage, submitter: gate, now: { clock.now })
        try outbox.keep(decision(1))
        let flush = Task { await outbox.flush() }
        for _ in 0..<200_000 where !gate.isHolding { await Task.yield() }
        try storage.save([])
        outbox.reload()
        #expect(outbox.entries.map(\.id) == ["id-1"], "not replaced while it is being sent")
        gate.open()
        await flush.value
        outbox.reload()
        #expect(outbox.entries.isEmpty)
    }

    @Test("a flush asked for while one goes on waits for it, and sends what was kept meanwhile before it returns")
    func joiningAFlush() async throws {
        let gate = GateSubmitter()
        let outbox = DecisionOutbox(storage: MemoryOutboxStorage(), submitter: gate, now: { clock.now })
        try outbox.keep(decision(1))
        let first = Task { await outbox.flush() }
        for _ in 0..<200_000 where !gate.isHolding { await Task.yield() }
        #expect(gate.isHolding, "the first send is held")
        try outbox.keep(decision(2))
        let second = Task { await outbox.flush() }
        for _ in 0..<200 { await Task.yield() }
        gate.open()
        await first.value
        await second.value
        #expect(gate.sent == ["id-1", "id-2"], "the one kept meanwhile did not wait for the next tick")
        #expect(outbox.entries.isEmpty)
    }

    @Test("which device signed what is kept: removing a device that signed a kept decision is what the outbox tells apart from removing another")
    func signedBy() throws {
        let (outbox, _, _) = outbox([])
        #expect(!outbox.isSigned(by: "d-1"), "nothing kept")
        try outbox.keep(decision(1))
        #expect(outbox.isSigned(by: "d-1"))
        #expect(!outbox.isSigned(by: "d-2"), "another device of the person signed nothing that is kept")
    }

    @Test("a flush that started meanwhile does not let go of what another flush took before that one's desk is read")
    func takenItemsAreEachFlushs() async throws {
        let outbox = DecisionOutbox(storage: MemoryOutboxStorage(), submitter: ScriptedSubmitter([took, took]), now: { clock.now })
        let held = HeldRead()
        outbox.didTake = { await held.wait() }
        try outbox.keep(decision(1))
        let first = Task { await outbox.flush() }
        for _ in 0..<200_000 where !held.isWaiting { await Task.yield() }
        #expect(held.isWaiting, "the first flush has taken its decision and is reading the desk")
        outbox.didTake = {}
        try outbox.keep(decision(2))
        await outbox.flush()
        #expect(outbox.justTaken == ["acme:app:r-1"], "the second flush took r-2 and let go of it: r-1 stays off the pile until its own desk read ends")
        held.release()
        await first.value
        #expect(outbox.justTaken.isEmpty)
    }

    @Test("a send that is cut short is not a failed try: nothing is counted, nothing is waited for")
    func cutShort() async throws {
        let outbox = DecisionOutbox(storage: MemoryOutboxStorage(), submitter: SlowSubmitter(), now: { clock.now })
        try outbox.keep(decision(1))
        let flush = Task { await outbox.flush() }
        for _ in 0..<200_000 where outbox.sending.isEmpty { await Task.yield() }
        #expect(outbox.sending.count == 1, "it is being sent")
        flush.cancel()
        await flush.value
        #expect(outbox.entries.count == 1 && outbox.entries[0].attempts == 0 && outbox.entries[0].failure == nil)
        #expect(outbox.entries[0].isDue(at: clock.now), "and it is due at once")
        #expect(outbox.sending.isEmpty)
    }

    @Test("when a flush is due again, and when it is not: a retry's time, now for what was cut short, nothing for what waits for the owner")
    func retryTime() async throws {
        let (outbox, _, _) = outbox([failing(URLError(.timedOut))])
        #expect(outbox.retryAt == nil, "nothing kept")
        try outbox.keep(decision(1))
        #expect(outbox.retryAt == clock.now, "due now")
        await outbox.flush()
        #expect(outbox.retryAt == clock.now.addingTimeInterval(5))
        try outbox.keep(decision(2))
        #expect(outbox.retryAt == clock.now, "the one not yet tried is due now, before the retry")
        let (stopped, _, _) = self.outbox([failing(NodeError.http(status: 409, message: "no"))])
        try stopped.keep(decision(3))
        await stopped.flush()
        #expect(stopped.retryAt == nil, "it stopped for good: only the owner can go on")
    }

    @Test("a run that sent some of the kept decisions while this one was suspended is taken in: the entries are read again")
    func reloading() async throws {
        let storage = MemoryOutboxStorage()
        let (outbox, _, _) = outbox([], storage: storage)
        try outbox.keep(decision(1))
        try outbox.keep(decision(2))
        let woken = DecisionOutbox(storage: storage, submitter: ScriptedSubmitter([took, took]), now: { clock.now })
        await woken.flush()
        #expect(outbox.entries.count == 2, "this one has not heard")
        outbox.reload()
        #expect(outbox.entries.isEmpty && outbox.decidedItemIDs.isEmpty)
    }

    @Test("an item has one decision kept; the items decided are the ones the pile leaves out")
    func decided() throws {
        let (outbox, _, _) = outbox([])
        try outbox.keep(decision(1))
        #expect(throws: OutboxError.alreadyDecided) { try outbox.keep(decision(1)) }
        try outbox.keep(decision(2))
        #expect(outbox.decidedItemIDs == ["acme:app:r-1", "acme:app:r-2"])
    }

    @Test("a decision the node takes leaves, whichever 2xx it answers, and what the node answers in words nobody reads is a 2xx too")
    func taking() async throws {
        let (outbox, submitter, storage) = outbox([took, answer("applied"), failing(NodeError.unreadable("x"))])
        for number in 1...3 { try outbox.keep(decision(number)) }
        await outbox.flush()
        #expect(outbox.entries.isEmpty && storage.entries.isEmpty)
        #expect(submitter.sent.map(\.envelope.id) == ["id-1", "id-2", "id-3"])
    }

    @Test("it survives the app being closed: a new outbox on the same storage has the same entries")
    func relaunch() throws {
        let (outbox, _, storage) = outbox([])
        try outbox.keep(decision(1))
        let again = DecisionOutbox(storage: storage, submitter: ScriptedSubmitter([]))
        #expect(again.entries.map(\.id) == ["id-1"] && again.decidedItemIDs == ["acme:app:r-1"])
    }

    @Test("a send that did not get through keeps the decision, waits 5 seconds, and then sends the very same envelope again")
    func retry() async throws {
        let (outbox, submitter, _) = outbox([failing(URLError(.notConnectedToInternet)), failing(NodeError.http(status: 503, message: "busy")), took])
        try outbox.keep(decision(1))
        await outbox.flush()
        #expect(outbox.entries.count == 1 && outbox.entries[0].attempts == 1 && outbox.entries[0].failure?.isFinal == false)
        #expect(outbox.nextDue == clock.now.addingTimeInterval(5))
        await outbox.flush()
        #expect(submitter.sent.count == 1, "not before it is due")
        clock.advance(5)
        await outbox.flush()
        #expect(outbox.entries[0].attempts == 2 && outbox.nextDue == clock.now.addingTimeInterval(15))
        clock.advance(15)
        await outbox.flush()
        #expect(outbox.entries.isEmpty)
        let sent = submitter.sent
        #expect(sent.count == 3 && Set(sent.map(\.envelope.id)) == ["id-1"] && Set(sent.map(\.signature)) == ["c2ln"], "the same signed envelope each time")
    }

    @Test("a refusal that will not change stops, says why as the node said it, is not tried again, and the owner can let go of it")
    func finalRefusal() async throws {
        let (outbox, submitter, _) = outbox([failing(NodeError.http(status: 409, message: "refused: not yours")), took])
        try outbox.keep(decision(1))
        await outbox.flush()
        #expect(outbox.failed.map(\.id) == ["id-1"] && outbox.failed[0].failure?.reason == "refused: not yours")
        #expect(outbox.failed[0].nextAttemptAt == nil, "a refusal that will not change has no next try")
        clock.advance(3600)
        await outbox.flush()
        #expect(submitter.sent.count == 1, "not tried again by itself")
        #expect(outbox.nextDue == nil)
        #expect(outbox.discard("id-1"))
        #expect(outbox.entries.isEmpty && outbox.decidedItemIDs.isEmpty, "the item can be decided again")
    }

    @Test("a decision that is waiting to be sent or to be tried again is not let go of: only one that stopped is")
    func discardRules() async throws {
        let (outbox, _, _) = outbox([failing(URLError(.timedOut))])
        try outbox.keep(decision(1))
        #expect(!outbox.discard("id-1"), "nothing failed: it will be sent")
        await outbox.flush()
        #expect(!outbox.discard("id-1"), "it will be tried again: the node may take it any moment")
        #expect(!outbox.discard("id-9"), "no such entry")
        #expect(outbox.hasSignedEntries)
    }

    @Test("send again forgets the failure and sends now")
    func sendAgain() async throws {
        let (outbox, submitter, _) = outbox([failing(NodeError.http(status: 409, message: "no")), took])
        try outbox.keep(decision(1))
        await outbox.flush()
        await outbox.sendAgain("id-1")
        #expect(outbox.entries.isEmpty && submitter.sent.count == 2)
    }

    @Test("an ended sign-in stops everything and drops nothing, and what waited goes once it ran again")
    func signInEnded() async throws {
        let (outbox, submitter, _) = outbox([failing(NodeError.sessionExpired), took, took])
        try outbox.keep(decision(1))
        try outbox.keep(decision(2))
        await outbox.flush()
        #expect(outbox.needsSignIn && outbox.entries.count == 2 && submitter.sent.count == 1, "the second was not even tried")
        await outbox.flush()
        #expect(submitter.sent.count == 1, "nothing is sent until the sign-in ran")
        await outbox.signInRan()
        #expect(!outbox.needsSignIn && outbox.entries.isEmpty && submitter.sent.count == 3)
    }

    @Test("what sending says of a failure: which are tried again, which are final, and how long to wait")
    func classification() {
        #expect(SendFailure.of(NodeError.http(status: 503, message: nil)) == .retry(reason: "HTTP 503"))
        #expect(SendFailure.of(NodeError.http(status: 429, message: "slow")) == .retry(reason: "slow"))
        #expect(SendFailure.of(NodeError.http(status: 408, message: nil)) == .retry(reason: "HTTP 408"))
        for status in [425, 500, 502, 504, 599] {
            #expect(SendFailure.of(NodeError.http(status: status, message: nil)) == .retry(reason: "HTTP \(status)"), Comment(rawValue: "\(status)"))
        }
        for status in [400, 401, 403, 404, 409] {
            #expect(SendFailure.of(NodeError.http(status: status, message: "no")) == .final(reason: "no"), Comment(rawValue: "\(status)"))
        }
        #expect(SendFailure.of(NodeError.http(status: 401, message: "device not allowed")) == .final(reason: "device not allowed"), "a device that was removed signs for nobody: sending again cannot mend it")
        #expect(SendFailure.of(NodeError.notSignedIn) == .signInEnded && SendFailure.of(NodeError.sessionExpired) == .signInEnded)
        #expect(SendFailure.of(NodeError.unreadable("x")) == .taken)
        #expect([1, 2, 3, 4, 5, 9].map(SendFailure.delay(afterAttempts:)) == [5, 15, 60, 300, 300, 300])
    }

    @Test("the entry that is kept is what is sent, field for field")
    func roundTrip() throws {
        let entry = decision(1)
        #expect(entry.signed.envelope == DecisionEnvelope(
            id: "id-1", kind: .answer, org: "acme", team: "app", target: "r-1", text: "ok", choice: "a", account: "alice", at: "2026-11-27T10:00:00Z"
        ))
        #expect(entry.signed.signer == "d-1" && entry.signed.signature == "c2ln" && entry.signed.title == "Title 1" && entry.signed.choiceLabel == "Yes")
        let decoded = try JSONDecoder().decode(PendingDecision.self, from: JSONEncoder().encode(entry))
        #expect(decoded == entry)
    }
}
