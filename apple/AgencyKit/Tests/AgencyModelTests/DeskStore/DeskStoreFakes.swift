@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

/// The test doubles of the desk store, nested in its suite so that other files may keep their own of the same names.
extension DeskStoreTests {
    final class FakeSource: DeskSource, @unchecked Sendable {
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

    final class FakeSigner: DeskSigner, @unchecked Sendable {
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
    final class FakeSubmitter: DecisionSubmitter, @unchecked Sendable {
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

    final class MemoryStorage: DraftStorage, @unchecked Sendable {
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
    func waitUntil(_ condition: @Sendable () -> Bool, sourceLocation: SourceLocation = #_sourceLocation) async {
        for _ in 0..<200_000 where !condition() { await Task.yield() }
        #expect(condition(), "waited for a condition that never held", sourceLocation: sourceLocation)
    }

    actor Told {
        private(set) var count = 0
        private(set) var readsAtThatTime = 0
        func mark(readsSoFar: Int) { count += 1; readsAtThatTime = readsSoFar }
    }

    final class Counter: @unchecked Sendable {
        private let lock = NSLock()
        private var value = 0

        @discardableResult
        func next() -> Int { lock.withLock { value += 1; return value } }
        func set(_ new: Int) { lock.withLock { value = new } }
        var count: Int { lock.withLock { value } }
    }

    final class StoreHolder: @unchecked Sendable {
        @MainActor var store: DeskStore?
    }
}
