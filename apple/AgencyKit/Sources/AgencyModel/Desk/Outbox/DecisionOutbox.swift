import AgencyAPI
import Foundation
import Observation

/// Where the kept decisions live between launches. A write that fails throws: a decision that cannot be kept is not accepted.
public protocol OutboxStorage: Sendable {
    /// What is kept. An empty list means nothing is: a file that exists and cannot be read now throws, and is not taken for an empty one.
    func load() throws -> [PendingDecision]
    func save(_ entries: [PendingDecision]) throws
}

/// What sends a kept decision to the node. `NodeClient` is the real one.
public protocol DecisionSubmitter: Sendable {
    func submit(_ decision: SignedDecision, org: String, team: String) async throws -> DecisionResult
}

extension NodeClient: DecisionSubmitter {}

public enum OutboxError: Error, Equatable, Sendable {
    /// The decision could not be written to the device, so it was not accepted: nothing is promised that a kill could lose.
    case notKept
    /// The item already has a decision kept.
    case alreadyDecided
}

/// The decisions the owner has made and the node has not yet taken (agency#447). A decision is written here, signed, before the desk moves
/// on; it leaves only when the node has taken it (any 2xx), or when the owner discards a failed one. Sending is the same envelope each
/// time, so a lost answer and a resend never make two decisions.
@MainActor
@Observable
public final class DecisionOutbox {
    public private(set) var entries: [PendingDecision]
    /// The entries being sent now.
    public private(set) var sending: Set<String> = []
    /// The Access sign-in has ended: nothing is sent until `signInRan()`, and nothing is dropped.
    public private(set) var needsSignIn = false
    /// The kept decisions can be read. When they cannot (the file is protected until the first unlock, or a read failed), nothing new is kept:
    /// writing the file then would replace what is in it.
    public private(set) var isReadable: Bool
    /// The items whose decision the node took in the flush going on: they stay off the pile until the desk has been read again, so that a
    /// taken request does not come back for the length of a round trip.
    public private(set) var justTaken: Set<String> = []

    @ObservationIgnored private let storage: any OutboxStorage
    @ObservationIgnored private let submitter: any DecisionSubmitter
    @ObservationIgnored private let now: @Sendable () -> Date
    @ObservationIgnored private var isFlushing = false
    /// Set by a flush asked for while one goes on, so that the one going on makes another pass; the callers wait in `waiting`.
    @ObservationIgnored private var runsAgain = false
    @ObservationIgnored private var waiting: [CheckedContinuation<Void, Never>] = []
    /// Runs once after a flush in which the node took something: the desk is read again.
    @ObservationIgnored public var didTake: (@MainActor () async -> Void)?

    public init(storage: any OutboxStorage, submitter: any DecisionSubmitter, now: @escaping @Sendable () -> Date = { Date() }) {
        self.storage = storage
        self.submitter = submitter
        self.now = now
        let stored = try? storage.load()
        entries = stored.map { $0 } ?? []
        isReadable = stored != nil
    }

    /// The desk items that have a decision kept: they do not come back on the pile, and the counts leave them.
    public var decidedItemIDs: Set<String> { Set(entries.map(\.itemID)).union(justTaken) }

    /// Whether a signed decision is kept: the key that signed it must not be taken away (a device is not removed) until it is taken.
    public var hasSignedEntries: Bool { !entries.isEmpty }

    /// The entries that wait for the owner to decide what to do with them.
    public var failed: [PendingDecision] { entries.filter(\.isFinallyRefused) }

    /// When the next entry that waits for a retry is due, if any: the app wakes a flush then.
    public var nextDue: Date? {
        entries.filter { !$0.isFinallyRefused }.compactMap(\.nextAttemptAt).min()
    }

    // MARK: Keeping

    /// Keeps a signed decision: written before anything else happens, and refused when it cannot be written.
    public func keep(_ decision: PendingDecision) throws {
        if !isReadable { reload() }
        guard isReadable else { throw OutboxError.notKept }
        guard !entries.contains(where: { $0.itemID == decision.itemID }) else { throw OutboxError.alreadyDecided }
        let next = entries + [decision]
        do { try storage.save(next) } catch { throw OutboxError.notKept }
        entries = next
    }

    /// The owner lets go of a decision that the node refused for good: the item can be decided again. A decision that is still waiting to be
    /// sent or tried again, or is being sent, is not let go of: the node may take it any moment.
    @discardableResult
    public func discard(_ id: String) -> Bool {
        guard !sending.contains(id), let entry = entries.first(where: { $0.id == id }), entry.isFinallyRefused else { return false }
        let next = entries.filter { $0.id != id }
        guard (try? storage.save(next)) != nil else { return false }
        entries = next
        return true
    }

    /// Reads the kept decisions again: another run of the app (a wake that sent some while this one was suspended) may have changed them,
    /// and a file that could not be read may be readable now. Not while a flush is going on, which has its own view of them.
    public func reload() {
        guard !isFlushing else { return }
        guard let stored = try? storage.load() else {
            isReadable = false
            return
        }
        entries = stored
        isReadable = true
    }

    // MARK: Sending

    /// "Send again": the same envelope, now.
    public func sendAgain(_ id: String) async {
        guard !sending.contains(id), let index = entries.firstIndex(where: { $0.id == id }) else { return }
        entries[index].readyToSendAgain()
        persist()
        await flush()
    }

    /// The Access sign-in ran: what waited for it goes now.
    public func signInRan() async {
        needsSignIn = false
        await flush()
    }

    /// Sends every entry that is due. One flush at a time: a call while one goes on waits for it, and what was kept or became due
    /// meanwhile is sent in another pass before it returns. An ended sign-in stops it and the entries stay.
    public func flush() async {
        if isFlushing {
            runsAgain = true
            await withCheckedContinuation { waiting.append($0) }
            return
        }
        isFlushing = true
        var tookSome = false
        repeat {
            runsAgain = false
            if await pass() { tookSome = true }
        } while runsAgain
        isFlushing = false
        if tookSome {
            await didTake?()
            justTaken = []
        }
        let joined = waiting
        waiting = []
        for continuation in joined { continuation.resume() }
    }

    /// One try of every entry that is due; whether the node took any.
    private func pass() async -> Bool {
        let due = entries.filter { $0.isDue(at: now()) }.map(\.id)
        var tookSome = false
        for id in due {
            guard !needsSignIn, let entry = entries.first(where: { $0.id == id }), entry.isDue(at: now()) else { continue }
            if await send(entry) { tookSome = true }
        }
        return tookSome
    }

    /// Whether the node took it.
    private func send(_ entry: PendingDecision) async -> Bool {
        sending.insert(entry.id)
        defer { sending.remove(entry.id) }
        do {
            _ = try await submitter.submit(entry.signed, org: entry.org, team: entry.team)
            took(entry.id)
            return true
        } catch {
            switch SendFailure.of(error) {
            case .taken:
                took(entry.id)
                return true
            case .signInEnded: needsSignIn = true
            case .retry(let reason): failed(entry.id, Failure(reason: reason, isFinal: false))
            case .final(let reason): failed(entry.id, Failure(reason: reason, isFinal: true))
            }
            return false
        }
    }

    private typealias Failure = PendingDecision.Failure

    private func took(_ id: String) {
        if let entry = entries.first(where: { $0.id == id }) { justTaken.insert(entry.itemID) }
        entries.removeAll { $0.id == id }
        persist()
    }

    private func failed(_ id: String, _ failure: Failure) {
        guard let index = entries.firstIndex(where: { $0.id == id }) else { return }
        entries[index].failed(failure, retryAt: now().addingTimeInterval(SendFailure.delay(afterAttempts: entries[index].attempts + 1)))
        persist()
    }

    /// A write that fails here loses nothing that was kept: the entries are in memory and were written before; the next change writes again.
    private func persist() {
        try? storage.save(entries)
    }
}

/// Kept in memory: for tests.
public final class MemoryOutboxStorage: OutboxStorage, @unchecked Sendable {
    private let lock = NSLock()
    private var stored: [PendingDecision]
    public var failsToWrite = false
    public var failsToRead = false

    public init(_ entries: [PendingDecision] = []) { stored = entries }

    /// What is kept, for a test to look at.
    public var entries: [PendingDecision] { lock.withLock { stored } }

    public func load() throws -> [PendingDecision] {
        try lock.withLock {
            if failsToRead { throw OutboxError.notKept }
            return stored
        }
    }

    public func save(_ entries: [PendingDecision]) throws {
        try lock.withLock {
            if failsToWrite { throw OutboxError.notKept }
            stored = entries
        }
    }
}
