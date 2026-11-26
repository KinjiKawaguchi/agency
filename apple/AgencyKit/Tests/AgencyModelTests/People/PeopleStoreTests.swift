@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

private final class FakeTeams: TeamSnapshotSource, @unchecked Sendable {
    private let lock = NSLock()
    private var meAnswer: Result<Me, Error>
    private var snapshots: [String: Result<TeamSnapshot, Error>] = [:]

    init(me: Result<Me, Error>) { meAnswer = me }

    func set(me: Result<Me, Error>) { lock.withLock { meAnswer = me } }

    func set(_ snapshot: Result<TeamSnapshot, Error>, for key: String) { lock.withLock { snapshots[key] = snapshot } }

    func me() async throws -> Me { try lock.withLock { try meAnswer.get() } }

    func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot {
        try lock.withLock { try (snapshots["\(org)/\(team)"] ?? .success(Fixture.decode(TeamSnapshot.self, "team-snapshot-local"))).get() }
    }
}

private func me(_ json: String) -> Me { try! JSONDecoder().decode(Me.self, from: Data(json.utf8)) }

private let twoTeams = me("""
{"account":{"id":"alice","label":"ALICE","fingerprint":"f"},"funding":[],"orgs":[
 {"id":"acme","isRoot":true,"label":"Acme","teams":[{"id":"app","label":"app","location":"local","viewer":"owner"},{"id":"docs","label":"docs","location":"local","viewer":"owner"}]},
 {"id":"zed","isRoot":false,"label":"Zed","teams":[{"id":"ops","label":"ops","location":"remote","viewer":"owner"}]}]}
""")

@MainActor
@Suite("PeopleStore")
struct PeopleStoreTests {
    private typealias T = OverviewTeams

    @Test("lists the teams in the order the node gives them")
    func order() async {
        let source = FakeTeams(me: .success(twoTeams))
        source.set(.success(T.snapshot(org: "acme", team: "app")), for: "acme/app")
        source.set(.success(T.snapshot(org: "acme", team: "docs")), for: "acme/docs")
        source.set(.success(T.snapshot(org: "zed", team: "ops")), for: "zed/ops")
        let store = PeopleStore(source: source)
        await store.refresh()
        #expect(store.loading == .loaded)
        #expect(store.sections.map(\.id) == ["acme/app", "acme/docs", "zed/ops"])
    }

    @Test("a team that cannot be read is listed as that, and the others are not hidden")
    func oneFails() async {
        let source = FakeTeams(me: .success(twoTeams))
        source.set(.success(T.snapshot(org: "acme", team: "app")), for: "acme/app")
        source.set(.failure(NodeError.http(status: 502, message: "node away")), for: "acme/docs")
        source.set(.success(T.snapshot(org: "zed", team: "ops")), for: "zed/ops")
        let store = PeopleStore(source: source)
        await store.refresh()
        #expect(store.sections.map(\.id) == ["acme/app", "zed/ops"])
        #expect(store.failures == ["acme/docs": "node away"])
        #expect(!store.needsSignIn)
    }

    @Test("an ended sign-in is a sign-in to run again, whether the account or a team's read says so")
    func signInEnded() async {
        let ended = FakeTeams(me: .failure(NodeError.sessionExpired))
        let store = PeopleStore(source: ended)
        await store.refresh()
        #expect(store.needsSignIn)
        #expect(store.loading == .failed(Ja.Desk.signInEnded))

        let midway = FakeTeams(me: .success(twoTeams))
        midway.set(.failure(NodeError.sessionExpired), for: "acme/docs")
        let second = PeopleStore(source: midway)
        await second.refresh()
        #expect(second.needsSignIn)
    }

    @Test("a team that was read keeps its last answer when a later read fails, and a team gone from the account leaves")
    func keepsLastAnswer() async {
        let source = FakeTeams(me: .success(twoTeams))
        for (org, team) in [("acme", "app"), ("acme", "docs"), ("zed", "ops")] {
            source.set(.success(T.snapshot(org: org, team: team)), for: "\(org)/\(team)")
        }
        let store = PeopleStore(source: source)
        await store.refresh()
        #expect(store.sections.count == 3)
        source.set(.failure(NodeError.http(status: 500, message: "down")), for: "acme/docs")
        await store.refresh()
        #expect(store.sections.map(\.id).contains("acme/docs"))
        #expect(store.failures.isEmpty)
        source.set(me: .success(me(#"{"account":{"id":"a","label":"A","fingerprint":"f"},"funding":[],"orgs":[]}"#)))
        await store.refresh()
        #expect(store.sections.isEmpty)
    }

    @Test("a failed account read says why")
    func accountFails() async {
        let store = PeopleStore(source: FakeTeams(me: .failure(NodeError.http(status: 500, message: "down"))))
        await store.refresh()
        #expect(store.loading == .failed("down"))
        #expect(store.sections.isEmpty)
    }
}
