@testable import AgencyAPI
@testable import AgencyModel
import AgencyWidget
import Foundation
import Testing

private final class RecordingSink: WidgetFileSink {
    var written: [WidgetSnapshot] = []
    var reloads = 0
    func write(_ snapshot: WidgetSnapshot) throws { written.append(snapshot) }
    func clear() {}
    func reloadWidgets() { reloads += 1 }
}

private struct MemoryStorage: WidgetPreferencesStorage {
    let preferences: WidgetPreferences
    func load() -> WidgetPreferences { preferences }
    func save(_: WidgetPreferences) {}
}

/// Answers from what it is given, per team; a team named in `failing` answers with a failure.
private final class FakeSource: TeamSnapshotSource, @unchecked Sendable {
    var me: Result<Me, Error> = .success(Fixture.decode(Me.self, "me"))
    var snapshot: TeamSnapshot = Teams.snapshot()
    var failing: Set<String> = []
    private let lock = NSLock()
    private var _reads = 0
    /// How many times a team was asked for.
    var reads: Int { lock.withLock { _reads } }

    func me() async throws -> Me { try me.get() }
    func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot {
        lock.withLock { _reads += 1 }
        if failing.contains(team) { throw NodeError.http(status: 503, message: "not ready") }
        return snapshot
    }
}

@MainActor
@Suite("Desk background refresh")
struct DeskBackgroundRefreshTests {
    private let now = Date(timeIntervalSince1970: 1_795_521_600)
    private let sink = RecordingSink()
    private let source = FakeSource()

    private func refresh(showsTitles: Bool = true) -> DeskBackgroundRefresh {
        let preferences = WidgetPreferencesStore(storage: MemoryStorage(preferences: WidgetPreferences(showsTitles: showsTitles)))
        return DeskBackgroundRefresh(source: source, publisher: WidgetPublisher(host: "node.example", preferences: preferences, sink: sink))
    }

    @Test("the desk is read once and the widgets' file is written from it, and the widgets are told")
    func writes() async throws {
        let outcome = await refresh().run(now: now)
        #expect(outcome == .written)
        let snapshot = try #require(sink.written.first)
        #expect(snapshot.host == "node.example")
        #expect(snapshot.writtenAt == now)
        #expect(snapshot.waitingCount == Desk.openItems([Team(snapshot: source.snapshot)]).filter(\.isMine).count)
        #expect(snapshot.waitingCount > 0)
        #expect(sink.reloads == 1)
    }

    @Test("an empty desk is written as nothing waiting, so a widget that showed requests does not keep showing them")
    func emptyDesk() async throws {
        source.snapshot = Teams.snapshot(mine: [], waitingOnViewer: [])
        _ = await refresh().run(now: now)
        let snapshot = try #require(sink.written.first)
        #expect(snapshot.waitingCount == 0)
        #expect(snapshot.requests.isEmpty)
    }

    @Test("an account that cannot be read writes nothing")
    func noAccount() async {
        source.me = .failure(NodeError.sessionExpired)
        let outcome = await refresh().run(now: now)
        #expect(outcome == .notRead)
        #expect(sink.written.isEmpty && sink.reloads == 0)
    }

    @Test("one team that cannot be read writes nothing: a number made of the teams that answered would be wrong")
    func oneTeamFails() async {
        let two = """
        {"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[{"id":"acme","isRoot":true,"label":"Acme","teams":[
        {"id":"app","label":"app","location":"local","viewer":"owner"},{"id":"web","label":"web","location":"remote","viewer":"owner"}]}]}
        """
        source.me = .success(try! JSONDecoder().decode(Me.self, from: Data(two.utf8)))
        source.failing = ["web"]
        let outcome = await refresh().run(now: now)
        #expect(outcome == .notRead)
        #expect(sink.written.isEmpty)
    }

    @Test("a team that is not ready is asked once, not again after waits: a background run has seconds")
    func notWaitedFor() async {
        source.failing = ["app"]
        _ = await refresh().run(now: now)
        #expect(source.reads == 1, "the standard policy would ask six times, after 1, 2, 4, 8 and 15 seconds")
    }

    @Test("the owner's choice about titles is kept, and so is what the node says about them")
    func titles() async throws {
        _ = await refresh(showsTitles: false).run(now: now)
        #expect(try #require(sink.written.first).requests.allSatisfy { $0.title == nil })
        let another = RecordingSink()
        let hiding = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"hidesTitles":true,"orgs":[{"id":"acme","isRoot":true,"label":"Acme","teams":[{"id":"app","label":"app","location":"local","viewer":"owner"}]}]}"#
        source.me = .success(try JSONDecoder().decode(Me.self, from: Data(hiding.utf8)))
        let preferences = WidgetPreferencesStore(storage: MemoryStorage(preferences: WidgetPreferences(showsTitles: true)))
        _ = await DeskBackgroundRefresh(source: source, publisher: WidgetPublisher(host: "h", preferences: preferences, sink: another)).run(now: now)
        #expect(try #require(another.written.first).requests.allSatisfy { $0.title == nil }, "the node's hidesTitles wins over the switch")
    }
}
