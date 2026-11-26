@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

private final class FakeTeams: TeamSnapshotSource, @unchecked Sendable {
    private let lock = NSLock()
    private var meAnswer: Result<Me, Error>
    private var snapshots: [String: Result<TeamSnapshot, Error>] = [:]
    private var scripted: [String: (result: Result<TeamSnapshot, Error>, left: Int)] = [:]

    init(me: Result<Me, Error>) { meAnswer = me }

    func set(me: Result<Me, Error>) { lock.withLock { meAnswer = me } }

    func set(_ snapshot: Result<TeamSnapshot, Error>, for key: String) { lock.withLock { snapshots[key] = snapshot } }

    /// Fails with `snapshot` this many times, then answers as usual.
    func set(_ snapshot: Result<TeamSnapshot, Error>, for key: String, times: Int) {
        lock.withLock { scripted[key] = (snapshot, times) }
    }

    func me() async throws -> Me { try lock.withLock { try meAnswer.get() } }

    func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot {
        try lock.withLock {
            let key = "\(org)/\(team)"
            if let one = scripted[key], one.left > 0 {
                scripted[key] = (one.result, one.left - 1)
                return try one.result.get()
            }
            return try (snapshots[key] ?? .success(OverviewTeams.snapshot(org: org, team: team))).get()
        }
    }
}

/// The waits a policy was asked to make.
private final class WaitLog: @unchecked Sendable {
    private let lock = NSLock()
    private var _values: [TimeInterval] = []
    var values: [TimeInterval] { lock.withLock { _values } }
    func add(_ seconds: TimeInterval) { lock.withLock { _values.append(seconds) } }
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
        let store = PeopleStore(source: source, retry: .never)
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
        let store = PeopleStore(source: source, retry: .never)
        await store.refresh()
        #expect(store.sections.map(\.id) == ["acme/app", "zed/ops"])
        #expect(store.failures == ["acme/docs": "node away"])
        #expect(!store.needsSignIn)
    }

    @Test("an ended sign-in is a sign-in to run again, whether the account or a team's read says so")
    func signInEnded() async {
        let ended = FakeTeams(me: .failure(NodeError.sessionExpired))
        let store = PeopleStore(source: ended, retry: .never)
        await store.refresh()
        #expect(store.needsSignIn)
        #expect(store.loading == .failed(Ja.Desk.signInEnded))

        let midway = FakeTeams(me: .success(twoTeams))
        midway.set(.failure(NodeError.sessionExpired), for: "acme/docs")
        let second = PeopleStore(source: midway, retry: .never)
        await second.refresh()
        #expect(second.needsSignIn)
    }

    @Test("a team that was read keeps its last answer when a later read fails, and a team gone from the account leaves")
    func keepsLastAnswer() async {
        let source = FakeTeams(me: .success(twoTeams))
        for (org, team) in [("acme", "app"), ("acme", "docs"), ("zed", "ops")] {
            source.set(.success(T.snapshot(org: org, team: team)), for: "\(org)/\(team)")
        }
        let store = PeopleStore(source: source, retry: .never)
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

    @Test("a team whose view is not ready yet is asked again, and is a failure only when it keeps failing")
    func notReady() async {
        let waits = WaitLog()
        let source = FakeTeams(me: .success(twoTeams))
        source.set(.failure(NodeError.http(status: 503, message: "the board has not been read yet")), for: "acme/docs", times: 2)
        let store = PeopleStore(source: source, retry: RetryPolicy(delays: [1, 2, 4], wait: { waits.add($0) }))
        await store.refresh()
        #expect(waits.values == [1, 2])
        #expect(store.failures.isEmpty)
        #expect(store.sections.map(\.id).contains("acme/app"))
        #expect(store.sections.count == 3)

        let stuck = FakeTeams(me: .success(twoTeams))
        stuck.set(.failure(NodeError.http(status: 503, message: "the board has not been read yet")), for: "acme/docs")
        let second = PeopleStore(source: stuck, retry: RetryPolicy(delays: [1], wait: { _ in }))
        await second.refresh()
        #expect(second.failures == ["acme/docs": "the board has not been read yet"])
    }

    @Test("a read that was cancelled says nothing about the team")
    func cancelledRead() async {
        let source = FakeTeams(me: .success(twoTeams))
        source.set(.failure(CancellationError()), for: "acme/docs")
        let store = PeopleStore(source: source, retry: .never)
        await store.refresh()
        #expect(store.failures.isEmpty)
        #expect(store.sections.map(\.id) == ["acme/app", "zed/ops"])
    }

    @Test("a failed account read says why")
    func accountFails() async {
        let store = PeopleStore(source: FakeTeams(me: .failure(NodeError.http(status: 500, message: "down"))), retry: .never)
        await store.refresh()
        #expect(store.loading == .failed("down"))
        #expect(store.sections.isEmpty)
    }
}
