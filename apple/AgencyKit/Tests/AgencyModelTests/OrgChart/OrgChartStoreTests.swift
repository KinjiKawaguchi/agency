@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

private final class FakeChart: OrgChartSource, @unchecked Sendable {
    private let lock = NSLock()
    private var meAnswer: Result<Me, Error>
    private var definitions: [String: Result<OrgDefinition, Error>] = [:]
    private var snapshots: [String: Result<TeamSnapshot, Error>] = [:]

    init(me: Result<Me, Error>) { meAnswer = me }

    func set(me: Result<Me, Error>) { lock.withLock { meAnswer = me } }
    func set(definition: Result<OrgDefinition, Error>, for org: String) { lock.withLock { definitions[org] = definition } }
    func set(_ snapshot: Result<TeamSnapshot, Error>, for key: String) { lock.withLock { snapshots[key] = snapshot } }

    func me() async throws -> Me { try lock.withLock { try meAnswer.get() } }

    func orgDefinition(org: String) async throws -> OrgDefinition {
        try lock.withLock { try (definitions[org] ?? .success(OrgFixtures.acme)).get() }
    }

    func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot {
        try lock.withLock {
            try (snapshots["\(org)/\(team)"] ?? .success(OverviewTeams.snapshot(org: org, team: team, people: [OverviewTeams.person("a", "working")], mine: team == "app" ? ["r-1", "r-2"] : []))).get()
        }
    }
}

private func me(_ json: String) -> Me { try! JSONDecoder().decode(Me.self, from: Data(json.utf8)) }
private let account = #""account":{"id":"alice","label":"ALICE","fingerprint":"f"},"funding":[]"#
private let team = #"{"id":"app","label":"App","location":"local","viewer":"owner"},{"id":"site","label":"Site","location":"local","viewer":"owner"}"#
private let twoOrgs = me(#"{\#(account),"orgs":[{"id":"acme","isRoot":true,"label":"Acme","teams":[\#(team)]},{"id":"lab","isRoot":true,"label":"Lab","teams":[]}]}"#)
private let oneOrg = me(#"{\#(account),"orgs":[{"id":"acme","isRoot":true,"label":"Acme","teams":[\#(team)]}]}"#)

@MainActor
@Suite("OrgChartStore")
struct OrgChartStoreTests {
    @Test("with several orgs and none chosen every one is drawn, under the account, the org first")
    func allOrgs() async {
        let store = OrgChartStore(source: FakeChart(me: .success(twoOrgs)), retry: .never)
        await store.refresh()
        #expect(store.loading == .loaded)
        #expect(store.isAll)
        #expect(store.title == "あなたのすべての組織")
        let rows = store.rows
        guard case .top(let kind, let name, _) = rows[0].content else { Issue.record("no top"); return }
        #expect([kind, name] == ["アカウント", "ALICE"])
        #expect(rows.map(\.depth).prefix(4) == [0, 1, 2, 3])
        if case .org(let node) = rows[1].content { #expect(node.mine == 2) } else { Issue.record("no org row") }
    }

    @Test("one org is drawn when the account has one, as its own chart, with the org at the top")
    func oneOrgOnly() async {
        let store = OrgChartStore(source: FakeChart(me: .success(oneOrg)), retry: .never)
        await store.refresh()
        #expect(!store.isAll)
        #expect(store.title == "Acme の組織")
        guard case .top(let kind, let name, _) = store.rows[0].content else { Issue.record("no top"); return }
        #expect([kind, name] == ["組織", "Acme"])
        #expect(store.rows.map(\.depth).max() == 3)
    }

    @Test("choosing an org draws its chart, choosing all goes back, and a choice that is not on offer changes nothing")
    func choosing() async {
        let store = OrgChartStore(source: FakeChart(me: .success(twoOrgs)), retry: .never)
        await store.refresh()
        store.choose("acme")
        #expect(!store.isAll)
        #expect(store.org?.id == "acme")
        store.choose("nowhere")
        #expect(store.selection == "acme")
        #expect(store.org?.id == "acme")
        store.choose(OrgChartStore.all)
        #expect(store.isAll)
        let single = OrgChartStore(source: FakeChart(me: .success(oneOrg)), retry: .never)
        await single.refresh()
        single.choose(OrgChartStore.all)
        #expect(!single.isAll)
    }

    @Test("focusing on a department draws it at the top with what is below it; choosing another org clears the focus")
    func focusing() async {
        let store = OrgChartStore(source: FakeChart(me: .success(oneOrg)), retry: .never)
        await store.refresh()
        store.focus(on: "engineering")
        guard case .top(let kind, let name, let sub) = store.rows[0].content else { Issue.record("no top"); return }
        #expect([kind, name, sub ?? ""] == ["部署", "Engineering", "責任者は Alice"])
        #expect(!store.isEmptyDepartment)
        store.focus(on: "platform")
        #expect(store.rows.count == 2)
        store.focus(on: nil)
        #expect(store.rows.count > 5)
        let many = OrgChartStore(source: FakeChart(me: .success(twoOrgs)), retry: .never)
        await many.refresh()
        many.choose("acme")
        many.focus(on: "design")
        many.choose("lab")
        #expect(many.focus == nil)
    }

    @Test("a department with nobody in it says so")
    func emptyDepartment() async {
        let source = FakeChart(me: .success(oneOrg))
        source.set(definition: .success(OrgFixtures.definition(#"{"org":"acme","label":"Acme","departments":[{"id":"empty","label":"Empty","parent":null,"head":null}],"people":[],"teams":[]}"#)), for: "acme")
        let store = OrgChartStore(source: source, retry: .never)
        await store.refresh()
        store.focus(on: "empty")
        #expect(store.isEmptyDepartment)
    }

    @Test("an org whose definition cannot be read is drawn as such, and the others are still drawn")
    func definitionFails() async {
        let source = FakeChart(me: .success(twoOrgs))
        source.set(definition: .failure(NodeError.http(status: 500, message: "boom")), for: "lab")
        let store = OrgChartStore(source: source, retry: .never)
        await store.refresh()
        let states = store.rows.compactMap { row -> OrgNode.State? in if case .org(let node) = row.content { node.state } else { nil } }
        #expect(states == [.ready, .failed])
    }

    @Test("the one org's chart says when its definition could not be read, and a definition read before stays")
    func definitionKept() async {
        let source = FakeChart(me: .success(oneOrg))
        source.set(definition: .failure(NodeError.http(status: 500, message: "boom")), for: "acme")
        let store = OrgChartStore(source: source, retry: .never)
        await store.refresh()
        #expect(store.isOrgUnavailable)
        #expect(store.rows.isEmpty)
        source.set(definition: .success(OrgFixtures.acme), for: "acme")
        await store.refresh()
        #expect(!store.isOrgUnavailable)
        source.set(definition: .failure(NodeError.http(status: 500, message: "boom")), for: "acme")
        await store.refresh()
        #expect(!store.isOrgUnavailable)
        #expect(!store.rows.isEmpty)
    }

    @Test("a team that cannot be read is a box that says so, and a team that was read keeps its last answer")
    func teamFails() async {
        let source = FakeChart(me: .success(oneOrg))
        source.set(.failure(NodeError.http(status: 500, message: "down")), for: "acme/site")
        let store = OrgChartStore(source: source, retry: .never)
        await store.refresh()
        let nodes = store.rows.compactMap { row -> TeamNode? in if case .team(let node) = row.content { node } else { nil } }
        #expect(nodes.map(\.place.id) == ["app", "site"])
        #expect(nodes[0].signals != nil)
        #expect(nodes[1].signals == nil)
        #expect(TeamBoxText(node: nodes[1]).unavailable == "このチームを読み込めませんでした")
        source.set(.failure(NodeError.http(status: 500, message: "down")), for: "acme/app")
        await store.refresh()
        let again = store.rows.compactMap { row -> TeamNode? in if case .team(let node) = row.content { node } else { nil } }
        #expect(again[0].signals != nil)
    }

    @Test("a team whose view is not ready is asked again, and is a box that says so only when it keeps failing")
    func notReady() async {
        let source = FakeChart(me: .success(oneOrg))
        source.set(.failure(NodeError.http(status: 503, message: "the board has not been read yet")), for: "acme/site")
        let store = OrgChartStore(source: source, retry: RetryPolicy(delays: [1, 2], wait: { _ in }))
        await store.refresh()
        let site = store.rows.compactMap { row -> TeamNode? in if case .team(let node) = row.content, node.id == "site" { node } else { nil } }.first
        #expect(site?.signals == nil)
        #expect(site?.isLoading == false)
    }

    @Test("an ended sign-in is a sign-in to run again; the account's orgs that change are followed")
    func signInAndChanges() async {
        let ended = OrgChartStore(source: FakeChart(me: .failure(NodeError.sessionExpired)), retry: .never)
        await ended.refresh()
        #expect(ended.needsSignIn)
        #expect(ended.loading == .failed(Ja.Desk.signInEnded))

        let source = FakeChart(me: .success(twoOrgs))
        let store = OrgChartStore(source: source, retry: .never)
        await store.refresh()
        store.choose("lab")
        source.set(me: .success(oneOrg))
        await store.refresh()
        #expect(store.org?.id == "acme")
        #expect(!store.isAll)
    }

    @Test("one read at a time: a refresh asked while one runs waits for it")
    func oneReadAtATime() async {
        let store = OrgChartStore(source: FakeChart(me: .success(oneOrg)), retry: .never)
        async let first: Void = store.refresh()
        async let second: Void = store.refresh()
        _ = await (first, second)
        #expect(store.loading == .loaded)
    }
}

@Suite("Team box text")
struct TeamBoxTextTests {
    private func node(_ team: Team?, loading: Bool = false) -> TeamNode {
        OrgTreeBuilder.oneOrg(OrgFixtures.acme, teams: team.map { [$0] } ?? [], viewerAccount: "alice", loading: loading ? ["acme/app"] : [])
            .children.compactMap { child -> TeamNode? in
                if case .department(let department) = child { for case .team(let node) in department.children { return node } }
                return nil
            }.first!
    }

    @Test("a team waiting on the viewer says how many requests, in the overview's words, and how many work")
    func waiting() {
        let text = TeamBoxText(node: node(Teams.team()))
        #expect(text.waiting == "あなたを待っているリクエスト \(Teams.team().mine.count) 件")
        #expect(text.working == "メンバー 4 人のうち 2 人が作業中")
        #expect(text.tag == nil)
        #expect(text.unavailable == nil)
        #expect(text.oldData == nil)
    }

    @Test("a stuck team and an idle team carry the overview's tags")
    func tags() {
        let stuck = OverviewTeams.team(people: [OverviewTeams.person("a", "working")], work: [OverviewTeams.blocked("x#1", "external")])
        #expect(TeamBoxText(node: node(stuck)).tag == "止まっている")
        let idle = OverviewTeams.team(people: [OverviewTeams.person("a", "idle", status: "Nothing in Next")])
        #expect(TeamBoxText(node: node(idle)).tag == "動いていない")
        #expect(TeamBoxText(node: node(OverviewTeams.team())).working == "メンバーの報告がありません。")
    }

    @Test("a copy that could not be renewed carries the old-data mark, and a team that is loading says nothing is wrong")
    func marks() {
        let stale: [String: Any] = ["kind": "replica", "fetchedAt": "2026-11-24T10:00:00Z", "triedAt": "2026-11-24T12:00:00Z", "error": "refused"]
        #expect(TeamBoxText(node: node(OverviewTeams.team(source: stale))).oldData == "古いデータ")
        let fresh: [String: Any] = ["kind": "replica", "fetchedAt": "2026-11-24T10:00:00Z", "triedAt": "2026-11-24T10:00:00Z"]
        #expect(TeamBoxText(node: node(OverviewTeams.team(source: fresh))).oldData == nil)
        let loading = TeamBoxText(node: node(nil, loading: true))
        #expect(loading.unavailable == nil)
        #expect(loading.waiting == nil)
    }

    @Test("six faces, then how many more")
    func faces() {
        let many = OverviewTeams.team(people: (1...9).map { OverviewTeams.person("p\($0)", "working") })
        let text = TeamBoxText(node: node(many))
        #expect(text.faces.count == 6)
        #expect(text.more == "+3")
        #expect(TeamBoxText(node: node(OverviewTeams.team(people: [OverviewTeams.person("a", "idle")]))).more == nil)
    }
}
