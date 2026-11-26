import AgencyAPI
import Foundation

/// One shared read of the account and of every team's view, for the screens that all want the same data.
///
/// The desk, the overview, the participants screen and the costs screen each read every team, so
/// without this the app asks the node several times for the same thing and the screens can disagree for a
/// moment. A read made within `window` of the last one is answered from it, and a read that is already on
/// its way is shared, so two screens that ask at the same moment cause one request. A failed read is not
/// kept: the next ask tries again. After a decision (`invalidate()`) nothing older is handed out, and a
/// read that started before it does not become the cache.
///
/// It stands in for the real source (`TeamSnapshotSource`), so a screen reads through it without knowing.
public actor SnapshotCache: TeamSnapshotSource {
    private struct Entry<Value: Sendable> {
        let at: Date
        let value: Value
    }

    private let source: any TeamSnapshotSource
    private let window: TimeInterval
    private let clock: @Sendable () -> Date
    private var generation = 0
    private var cachedMe: Entry<Me>?
    private var pendingMe: Task<Me, Error>?
    private var cachedSnapshots: [String: Entry<TeamSnapshot>] = [:]
    private var pendingSnapshots: [String: Task<TeamSnapshot, Error>] = [:]

    /// - Parameters:
    ///   - window: how long an answer is reused; a few seconds, long enough for the screens that read together.
    ///   - clock: a parameter so tests can move time.
    public init(source: any TeamSnapshotSource, window: TimeInterval = 5, clock: @escaping @Sendable () -> Date = Date.init) {
        self.source = source
        self.window = window
        self.clock = clock
    }

    public func me() async throws -> Me {
        if let cachedMe, isFresh(cachedMe.at) { return cachedMe.value }
        if let pendingMe { return try await pendingMe.value }
        let source = source
        let started = generation
        let task = Task { try await source.me() }
        pendingMe = task
        defer { if generation == started { pendingMe = nil } }
        let value = try await task.value
        if generation == started { cachedMe = Entry(at: clock(), value: value) }
        return value
    }

    public func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot {
        let key = "\(org)/\(team)"
        if let cached = cachedSnapshots[key], isFresh(cached.at) { return cached.value }
        if let pending = pendingSnapshots[key] { return try await pending.value }
        let source = source
        let started = generation
        let task = Task { try await source.teamSnapshot(org: org, team: team) }
        pendingSnapshots[key] = task
        defer { if generation == started { pendingSnapshots[key] = nil } }
        let value = try await task.value
        if generation == started { cachedSnapshots[key] = Entry(at: clock(), value: value) }
        return value
    }

    /// Forgets everything read so far, and the reads that are on their way: call it after a decision, so
    /// the next read shows the node's state after it.
    public func invalidate() {
        generation += 1
        cachedMe = nil
        pendingMe = nil
        cachedSnapshots = [:]
        pendingSnapshots = [:]
    }

    private func isFresh(_ at: Date) -> Bool {
        let age = clock().timeIntervalSince(at)
        return age >= 0 && age < window
    }
}

/// What the screens read from: the account and the teams through the shared cache, and the rest (the
/// decision log, a team's token use) straight from the client.
public struct CachedSource<Client: DeskSource & TeamUsageSource>: DeskSource, OverviewSource {
    private let cache: SnapshotCache
    private let client: Client

    public init(cache: SnapshotCache, client: Client) {
        self.cache = cache
        self.client = client
    }

    public func me() async throws -> Me { try await cache.me() }
    public func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot { try await cache.teamSnapshot(org: org, team: team) }
    public func decisions(limit: Int) async throws -> DecisionLog { try await client.decisions(limit: limit) }
    public func usage(org: String, team: String, days: Int) async throws -> TokenUsage {
        try await client.usage(org: org, team: team, days: days)
    }
}
