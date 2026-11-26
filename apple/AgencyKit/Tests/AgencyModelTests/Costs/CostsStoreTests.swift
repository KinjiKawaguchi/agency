@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

private final class FakeCosts: OverviewSource, @unchecked Sendable {
    private let lock = NSLock()
    private var meAnswer: Result<Me, Error>
    private var snapshots: [String: Result<TeamSnapshot, Error>] = [:]
    private var usages: [String: Result<TokenUsage, Error>] = [:]
    private var asked: [String] = []
    private var gates: [CheckedContinuation<Void, Never>] = []
    private var heldDays: Int?

    init(me: Result<Me, Error>) { meAnswer = me }

    /// Holds the usage reads for this period until `release()`, so another period can be chosen meanwhile.
    func hold(days: Int) { lock.withLock { heldDays = days } }
    func release() { lock.withLock { let waiting = gates; gates = []; heldDays = nil; return waiting }.forEach { $0.resume() } }
    /// How many reads are waiting to be released.
    var holding: Int { lock.withLock { gates.count } }

    var usageAsked: [String] { lock.withLock { asked } }
    func set(me: Result<Me, Error>) { lock.withLock { meAnswer = me } }
    func set(_ snapshot: Result<TeamSnapshot, Error>, for key: String) { lock.withLock { snapshots[key] = snapshot } }
    func set(usage: Result<TokenUsage, Error>, for key: String) { lock.withLock { usages[key] = usage } }

    func me() async throws -> Me { try lock.withLock { try meAnswer.get() } }

    func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot {
        try lock.withLock { try (snapshots["\(org)/\(team)"] ?? .success(OverviewTeams.snapshot(org: org, team: team))).get() }
    }

    func usage(org: String, team: String, days: Int) async throws -> TokenUsage {
        // The answer is the one standing when the read starts; a held read delivers it late.
        let answer: Result<TokenUsage, Error> = lock.withLock {
            asked.append("\(org)/\(team)?days=\(days)")
            return usages["\(org)/\(team)"] ?? .success(Fixture.decode(TokenUsage.self, usageFixture))
        }
        if lock.withLock({ heldDays == days }) {
            await withCheckedContinuation { continuation in lock.withLock { gates.append(continuation) } }
        }
        return try answer.get()
    }
}

private func me(_ json: String) -> Me { try! JSONDecoder().decode(Me.self, from: Data(json.utf8)) }

private let account = #""account":{"id":"alice","label":"ALICE","fingerprint":"f"}"#
private let twoTeams = me(#"{\#(account),"funding":[{"id":"s","label":"Plan","kind":"subscription","windows":[]}],"orgs":[{"id":"acme","isRoot":true,"label":"Acme","teams":[{"id":"app","label":"app","location":"local","viewer":"owner"},{"id":"docs","label":"docs","location":"local","viewer":"owner"}]}]}"#)

@MainActor
@Suite("CostsStore")
struct CostsStoreTests {
    @Test("reads the funding sources, every team and its usage over 14 days to begin with")
    func reads() async {
        let source = FakeCosts(me: .success(twoTeams))
        let store = CostsStore(source: source)
        await store.refresh()
        #expect(store.loading == .loaded)
        #expect(store.fundingSummaries().map(\.label) == ["Plan"])
        #expect(store.teams.map(\.id) == ["acme/app", "acme/docs"])
        #expect(store.teams.allSatisfy { $0.report != nil })
        #expect(source.usageAsked.sorted() == ["acme/app?days=14", "acme/docs?days=14"])
    }

    @Test("choosing another period reads every team again for it, and only for a period on offer")
    func choosesPeriod() async {
        let source = FakeCosts(me: .success(twoTeams))
        let store = CostsStore(source: source)
        await store.refresh()
        await store.choose(days: 30)
        #expect(store.days == 30)
        #expect(source.usageAsked.filter { $0.hasSuffix("days=30") }.sorted() == ["acme/app?days=30", "acme/docs?days=30"])
        let before = source.usageAsked.count
        await store.choose(days: 30)
        await store.choose(days: 5)
        #expect(source.usageAsked.count == before)
        #expect(store.days == 30)
    }

    @Test("404 means the team reports no usage, 503 that it could not answer just now, anything else a failure")
    func outcomes() async {
        let source = FakeCosts(me: .success(twoTeams))
        source.set(usage: .failure(NodeError.http(status: 404, message: nil)), for: "acme/app")
        source.set(usage: .failure(NodeError.http(status: 503, message: "runtime busy")), for: "acme/docs")
        let store = CostsStore(source: source)
        await store.refresh()
        #expect(store.teams[0].usage == .unavailable)
        #expect(store.teams[1].usage == .couldNotAnswer("runtime busy"))
        source.set(usage: .failure(NodeError.http(status: 500, message: "boom")), for: "acme/docs")
        await store.refresh()
        #expect(store.teams[1].usage == .failed("boom"))
    }

    @Test("an answer for a period that is no longer the chosen one does not overwrite the new period's")
    func latePeriod() async throws {
        let source = FakeCosts(me: .success(twoTeams))
        let store = CostsStore(source: source)
        await store.refresh()
        source.set(usage: .success(Fixture.decode(TokenUsage.self, usageFixture)), for: "acme/docs")
        source.hold(days: 14)
        let late = Task { await store.refresh() }
        while source.holding < 2 { await Task.yield() }
        source.set(usage: .failure(NodeError.http(status: 404, message: nil)), for: "acme/app")
        await store.choose(days: 30)
        #expect(store.teams[0].usage == .unavailable)
        source.release()
        await late.value
        #expect(store.days == 30)
        #expect(store.teams[0].usage == .unavailable)
    }

    @Test("a team that cannot be read is not listed, and the others are")
    func teamFails() async {
        let source = FakeCosts(me: .success(twoTeams))
        source.set(.failure(NodeError.http(status: 502, message: "away")), for: "acme/docs")
        let store = CostsStore(source: source)
        await store.refresh()
        #expect(store.teams.map(\.id) == ["acme/app"])
        #expect(source.usageAsked == ["acme/app?days=14"])
    }

    @Test("a team that was read keeps its last answer when a later read fails")
    func keepsLastAnswer() async {
        let source = FakeCosts(me: .success(twoTeams))
        let store = CostsStore(source: source)
        await store.refresh()
        source.set(.failure(NodeError.http(status: 500, message: "down")), for: "acme/docs")
        await store.refresh()
        #expect(store.teams.map(\.id) == ["acme/app", "acme/docs"])
    }

    @Test("an ended sign-in is a sign-in to run again, from the account's read or a team's")
    func signInEnded() async {
        let ended = CostsStore(source: FakeCosts(me: .failure(NodeError.sessionExpired)))
        await ended.refresh()
        #expect(ended.needsSignIn)
        #expect(ended.loading == .failed(Ja.Desk.signInEnded))

        let source = FakeCosts(me: .success(twoTeams))
        source.set(.failure(NodeError.sessionExpired), for: "acme/docs")
        let midway = CostsStore(source: source)
        await midway.refresh()
        #expect(midway.needsSignIn)
    }

    @Test("a team that leaves the account leaves the screen")
    func goneTeams() async {
        let source = FakeCosts(me: .success(twoTeams))
        let store = CostsStore(source: source)
        await store.refresh()
        source.set(me: .success(me(#"{\#(account),"funding":[],"orgs":[]}"#)))
        await store.refresh()
        #expect(store.teams.isEmpty)
        #expect(store.funding.isEmpty)
    }
}
