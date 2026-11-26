@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

private final class FakeOverviewSource: OverviewSource, @unchecked Sendable {
    private let lock = NSLock()
    private var meAnswer: Result<Me, Error> = .success(Fixture.decode(Me.self, "me"))
    private var snapshots: [String: Result<TeamSnapshot, Error>] = [:]
    private var usages: [String: Result<TokenUsage, Error>] = [:]
    private var asked: [String] = []

    var usageAsked: [String] { lock.withLock { asked } }

    func set(me: Result<Me, Error>) { lock.withLock { meAnswer = me } }
    func set(snapshot: Result<TeamSnapshot, Error>, for key: String) { lock.withLock { snapshots[key] = snapshot } }
    func set(usage: Result<TokenUsage, Error>, for key: String) { lock.withLock { usages[key] = usage } }

    func me() async throws -> Me { try lock.withLock { try meAnswer.get() } }

    func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot {
        try lock.withLock { try (snapshots["\(org)/\(team)"] ?? .success(Fixture.decode(TeamSnapshot.self, "team-snapshot-local"))).get() }
    }

    func usage(org: String, team: String, days: Int) async throws -> TokenUsage {
        try lock.withLock {
            asked.append("\(org)/\(team)?days=\(days)")
            return try (usages["\(org)/\(team)"] ?? .success(Fixture.decode(TokenUsage.self, "usage"))).get()
        }
    }
}

private func me(_ json: String) -> Me { try! JSONDecoder().decode(Me.self, from: Data(json.utf8)) }

private let twoOrgs = me("""
{"account":{"id":"alice","label":"ALICE","fingerprint":"34750f98bd59fcfc946da45aaabe933b"},"funding":[],"orgs":[
 {"id":"acme","isRoot":true,"label":"Acme","teams":[{"id":"app","label":"app","location":"local","viewer":"owner"},{"id":"docs","label":"docs","location":"local","viewer":"owner"}]},
 {"id":"zed","isRoot":false,"label":"Zed","teams":[{"id":"ops","label":"ops","location":"remote","viewer":"owner"}]}]}
""")

@MainActor
@Suite("OverviewStore")
struct OverviewStoreTests {
    private typealias T = OverviewTeams

    @Test("reads the account, every team and each team's week of cost")
    func reads() async {
        let source = FakeOverviewSource()
        let store = OverviewStore(source: source)
        await store.refresh()
        #expect(store.loading == .loaded)
        let entries = store.sections.flatMap(\.teams)
        #expect(entries.map(\.ref.id) == ["app"])
        #expect(entries.allSatisfy { if case .ready = $0.state { true } else { false } })
        #expect(source.usageAsked == ["acme/app?days=7"])
        guard case .known(let cost) = store.weekCost(of: entries[0]) else { Issue.record("the cost was not read"); return }
        #expect(abs(cost - 13.3) < 0.0001)
    }

    @Test("orders the orgs where their most pressing team is, and says how many requests wait in how many teams")
    func ordersAndCounts() async {
        let source = FakeOverviewSource()
        source.set(me: .success(twoOrgs))
        source.set(snapshot: .success(T.snapshot(org: "acme", team: "app", people: [T.person("a", "working")])), for: "acme/app")
        source.set(snapshot: .success(T.snapshot(org: "acme", team: "docs", people: [T.person("a", "working")])), for: "acme/docs")
        source.set(snapshot: .success(T.snapshot(org: "zed", team: "ops", mine: ["r-1", "r-2"])), for: "zed/ops")
        let store = OverviewStore(source: source)
        await store.refresh()
        #expect(store.sections.map(\.org.id) == ["zed", "acme"])
        #expect(store.waiting.requests == 2)
        #expect(store.waiting.teams == 1)
    }

    @Test("a team that cannot be read is shown as that, and the others are not hidden")
    func oneFails() async {
        let source = FakeOverviewSource()
        source.set(me: .success(twoOrgs))
        source.set(snapshot: .failure(NodeError.http(status: 502, message: "node away")), for: "zed/ops")
        let store = OverviewStore(source: source)
        await store.refresh()
        let ops = store.sections.flatMap(\.teams).first { $0.ref.id == "ops" }
        #expect(ops?.state == .failed(reason: "node away", signInEnded: false))
        #expect(store.sections.flatMap(\.teams).contains { $0.ref.id == "app" && $0.summary != nil })
        #expect(store.loading == .loaded)
    }

    @Test("a team whose node cannot be reached, and so shows an old copy, is not asked for its cost")
    func staleCopyNotAsked() async {
        let source = FakeOverviewSource()
        let replica: [String: Any] = ["kind": "replica", "triedAt": "2026-11-24T12:00:00Z", "error": "refused"]
        source.set(snapshot: .success(T.snapshot(source: replica)), for: "acme/app")
        let store = OverviewStore(source: source)
        await store.refresh()
        #expect(source.usageAsked.isEmpty)
        #expect(store.weekCosts["acme/app"] == nil)
    }

    @Test("a team that does not report usage says so, and a failing read says it failed")
    func costOutcomes() async {
        let source = FakeOverviewSource()
        source.set(me: .success(twoOrgs))
        source.set(usage: .failure(NodeError.http(status: 404, message: nil)), for: "acme/app")
        source.set(usage: .failure(NodeError.http(status: 500, message: nil)), for: "acme/docs")
        let store = OverviewStore(source: source)
        await store.refresh()
        #expect(store.weekCosts["acme/app"] == WeekCost.none)
        #expect(store.weekCosts["acme/docs"] == .failed)
    }

    @Test("an ended sign-in is a sign-in to run again, not an empty overview")
    func signInEnded() async {
        let source = FakeOverviewSource()
        source.set(me: .failure(NodeError.sessionExpired))
        let store = OverviewStore(source: source)
        await store.refresh()
        #expect(store.needsSignIn)
        #expect(store.loading == .failed(Ja.Desk.signInEnded))
        #expect(store.sections.isEmpty)
    }

    @Test("a team that was read keeps its last answer when a later read fails")
    func keepsLastAnswer() async {
        let source = FakeOverviewSource()
        let store = OverviewStore(source: source)
        await store.refresh()
        source.set(snapshot: .failure(NodeError.http(status: 500, message: "down")), for: "acme/app")
        await store.refresh()
        #expect(store.sections.flatMap(\.teams).first?.summary != nil)
    }

    @Test("a team that is gone from the account leaves the screen")
    func dropsGoneTeams() async {
        let source = FakeOverviewSource()
        source.set(me: .success(twoOrgs))
        let store = OverviewStore(source: source)
        await store.refresh()
        source.set(me: .success(Fixture.decode(Me.self, "me")))
        await store.refresh()
        #expect(store.sections.flatMap(\.teams).map(\.key) == ["acme/app"])
        #expect(store.states.keys.sorted() == ["acme/app"])
    }

    @Test("the funding sources come from the account")
    func fundingSources() async {
        let source = FakeOverviewSource()
        let withFunding = me(#"{"account":{"id":"a","label":"A","fingerprint":"f"},"orgs":[],"funding":[{"id":"s","label":"Plan","kind":"apiKey","windows":[]}]}"#)
        source.set(me: .success(withFunding))
        let store = OverviewStore(source: source)
        await store.refresh()
        #expect(store.fundingSummaries().map(\.label) == ["Plan"])
        #expect(store.fundingSummaries()[0].windows.isEmpty)
    }
}
