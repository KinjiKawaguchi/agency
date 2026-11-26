@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

private final class CountingSource: TeamSnapshotSource, @unchecked Sendable {
    private let lock = NSLock()
    private var _meCalls = 0
    private var _snapshotCalls: [String] = []
    var meResult: Result<Me, Error> = .success(Fixture.decode(Me.self, "me"))
    var snapshotResult: Result<TeamSnapshot, Error> = .success(Fixture.decode(TeamSnapshot.self, "team-snapshot-local"))
    /// When set, the next call waits until `release()`.
    private var gates: [CheckedContinuation<Void, Never>] = []
    var holds = false

    var meCalls: Int { lock.withLock { _meCalls } }
    var snapshotCalls: [String] { lock.withLock { _snapshotCalls } }
    var waiting: Int { lock.withLock { gates.count } }
    func release() { lock.withLock { let all = gates; gates = []; return all }.forEach { $0.resume() } }

    func me() async throws -> Me {
        lock.withLock { _meCalls += 1 }
        try await hold()
        return try lock.withLock { try meResult.get() }
    }

    func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot {
        lock.withLock { _snapshotCalls.append("\(org)/\(team)") }
        try await hold()
        return try lock.withLock { try snapshotResult.get() }
    }

    private func hold() async throws {
        guard lock.withLock({ holds }) else { return }
        await withCheckedContinuation { continuation in lock.withLock { gates.append(continuation) } }
    }
}

private final class Clock: @unchecked Sendable {
    private let lock = NSLock()
    private var now = Date(timeIntervalSince1970: 1_795_521_600)
    var date: Date { lock.withLock { now } }
    func advance(_ seconds: TimeInterval) { lock.withLock { now = now.addingTimeInterval(seconds) } }
}

private final class FakeClient: DeskSource, TeamUsageSource, TeamSnapshotSource, @unchecked Sendable {
    private let lock = NSLock()
    private var _calls: [String] = []
    var calls: [String] { lock.withLock { _calls } }
    func me() async throws -> Me { lock.withLock { _calls.append("me") }; return Fixture.decode(Me.self, "me") }
    func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot { lock.withLock { _calls.append("snapshot") }; return Fixture.decode(TeamSnapshot.self, "team-snapshot-local") }
    func decisions(limit: Int) async throws -> DecisionLog { lock.withLock { _calls.append("decisions") }; return Fixture.decode(DecisionLog.self, "decision-log") }
    func usage(org: String, team: String, days: Int) async throws -> TokenUsage { lock.withLock { _calls.append("usage") }; return Fixture.decode(TokenUsage.self, "usage") }
}

@Suite("CachedSource")
struct CachedSourceTests {
    @Test("the account and the teams come through the cache; the decision log and the usage do not")
    func forwards() async throws {
        let client = FakeClient()
        let cache = SnapshotCache(source: client)
        let source = CachedSource(cache: cache, client: client)
        for _ in 0..<2 {
            _ = try await source.me()
            _ = try await source.teamSnapshot(org: "acme", team: "app")
            _ = try await source.decisions(limit: 20)
            _ = try await source.usage(org: "acme", team: "app", days: 7)
        }
        #expect(client.calls.filter { $0 == "me" }.count == 1)
        #expect(client.calls.filter { $0 == "snapshot" }.count == 1)
        #expect(client.calls.filter { $0 == "decisions" }.count == 2)
        #expect(client.calls.filter { $0 == "usage" }.count == 2)
    }
}

@Suite("SnapshotCache")
struct SnapshotCacheTests {
    private let source = CountingSource()
    private let clock = Clock()

    private func cache(window: TimeInterval = 5) -> SnapshotCache {
        let clock = clock
        return SnapshotCache(source: source, window: window, clock: { clock.date })
    }

    @Test("a read within the window is answered from the last one, for the account and for a team")
    func reuses() async throws {
        let cache = cache()
        _ = try await cache.me()
        _ = try await cache.me()
        _ = try await cache.teamSnapshot(org: "acme", team: "app")
        clock.advance(4.9)
        _ = try await cache.teamSnapshot(org: "acme", team: "app")
        _ = try await cache.me()
        #expect(source.meCalls == 1)
        #expect(source.snapshotCalls == ["acme/app"])
    }

    @Test("a read after the window asks again")
    func expires() async throws {
        let cache = cache()
        _ = try await cache.me()
        _ = try await cache.teamSnapshot(org: "acme", team: "app")
        clock.advance(5)
        _ = try await cache.me()
        _ = try await cache.teamSnapshot(org: "acme", team: "app")
        #expect(source.meCalls == 2)
        #expect(source.snapshotCalls.count == 2)
    }

    @Test("each team has its own entry")
    func perTeam() async throws {
        let cache = cache()
        _ = try await cache.teamSnapshot(org: "acme", team: "app")
        _ = try await cache.teamSnapshot(org: "acme", team: "web")
        _ = try await cache.teamSnapshot(org: "beta", team: "app")
        #expect(Set(source.snapshotCalls) == ["acme/app", "acme/web", "beta/app"])
    }

    @Test("two reads at the same moment cause one request")
    func sharesInFlight() async throws {
        source.holds = true
        let cache = cache()
        async let first = cache.teamSnapshot(org: "acme", team: "app")
        async let second = cache.teamSnapshot(org: "acme", team: "app")
        while source.waiting < 1 { await Task.yield() }
        try? await Task.sleep(for: .milliseconds(50))
        source.release()
        let (a, b) = try await (first, second)
        #expect(a == b)
        #expect(source.snapshotCalls == ["acme/app"])
    }

    @Test("two reads of the account at the same moment cause one request")
    func sharesMeInFlight() async throws {
        source.holds = true
        let cache = cache()
        async let first = cache.me()
        async let second = cache.me()
        while source.waiting < 1 { await Task.yield() }
        try? await Task.sleep(for: .milliseconds(50))
        source.release()
        _ = try await (first, second)
        #expect(source.meCalls == 1)
    }

    @Test("an answer for the account that started before a decision does not become the cache")
    func staleMeNotKept() async throws {
        source.holds = true
        let cache = cache()
        let early = Task { try await cache.me() }
        while source.waiting < 1 { await Task.yield() }
        await cache.invalidate()
        source.release()
        _ = try await early.value
        source.holds = false
        _ = try await cache.me()
        #expect(source.meCalls == 2)
    }

    @Test("a clock that went backwards does not make an old answer fresh")
    func clockBackwards() async throws {
        let cache = cache()
        _ = try await cache.me()
        clock.advance(-60)
        _ = try await cache.me()
        #expect(source.meCalls == 2)
    }

    @Test("a failure is given to everyone waiting and is not kept: the next read tries again")
    func failureNotKept() async throws {
        source.snapshotResult = .failure(NodeError.http(status: 503, message: "not ready"))
        let cache = cache()
        await #expect(throws: NodeError.http(status: 503, message: "not ready")) { try await cache.teamSnapshot(org: "acme", team: "app") }
        source.snapshotResult = .success(Fixture.decode(TeamSnapshot.self, "team-snapshot-local"))
        _ = try await cache.teamSnapshot(org: "acme", team: "app")
        #expect(source.snapshotCalls.count == 2)
        source.meResult = .failure(NodeError.sessionExpired)
        await #expect(throws: NodeError.sessionExpired) { try await cache.me() }
        source.meResult = .success(Fixture.decode(Me.self, "me"))
        _ = try await cache.me()
        #expect(source.meCalls == 2)
    }

    @Test("after invalidating, nothing older is handed out")
    func invalidates() async throws {
        let cache = cache()
        _ = try await cache.me()
        _ = try await cache.teamSnapshot(org: "acme", team: "app")
        await cache.invalidate()
        _ = try await cache.me()
        _ = try await cache.teamSnapshot(org: "acme", team: "app")
        #expect(source.meCalls == 2)
        #expect(source.snapshotCalls.count == 2)
    }

    @Test("a read that started before a decision does not become the cache")
    func staleReadNotKept() async throws {
        source.holds = true
        let cache = cache()
        let early = Task { try await cache.teamSnapshot(org: "acme", team: "app") }
        while source.waiting < 1 { await Task.yield() }
        await cache.invalidate()
        source.release()
        _ = try await early.value
        source.holds = false
        _ = try await cache.teamSnapshot(org: "acme", team: "app")
        #expect(source.snapshotCalls.count == 2, "the answer from before the decision must not be reused")
    }

    @Test("a read started after a decision is not joined to one that started before it")
    func newReadAfterInvalidate() async throws {
        source.holds = true
        let cache = cache()
        let before = Task { try await cache.teamSnapshot(org: "acme", team: "app") }
        while source.waiting < 1 { await Task.yield() }
        await cache.invalidate()
        let after = Task { try await cache.teamSnapshot(org: "acme", team: "app") }
        while source.waiting < 2 { await Task.yield() }
        source.release()
        _ = try await before.value
        _ = try await after.value
        #expect(source.snapshotCalls.count == 2)
    }
}
