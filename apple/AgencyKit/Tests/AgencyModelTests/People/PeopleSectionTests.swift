@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

@Suite("People section")
struct PeopleSectionTests {
    private let section = PeopleSection(team: Teams.team())
    private let now = Date(timeIntervalSince1970: 1_795_521_600)

    @Test("leaves the viewer out of the participants, and counts who works and who waits on the viewer")
    func counts() {
        #expect(section.cards.map(\.id) == ["lead", "backend", "frontend", "docs"])
        #expect(section.working == 2)
        #expect(section.waitingOnViewer == 1)
        #expect(section.title == "App チーム")
        #expect(section.summary == "参加者 4 人のうち 2 人が作業中です。 1 人の参加者があなたの返事を待っています。")
    }

    @Test("the summary says nothing of waiting when nobody waits on the viewer")
    func noWaiting() {
        let team = Teams.team(waitingOnViewer: [])
        #expect(PeopleSection(team: team).summary == "参加者 4 人のうち 2 人が作業中です。")
    }

    @Test("a participant who waits on the viewer says what for: the first request that stops them")
    func waitingFor() throws {
        let backend = try #require(section.cards.first { $0.id == "backend" })
        #expect(backend.presence == .needsYou)
        #expect(backend.waitingForTitle == "Which database should the app use in production?")
        #expect(backend.stateTag == "あなたの番")
        let lead = try #require(section.cards.first { $0.id == "lead" })
        #expect(lead.waitingForTitle == nil)
        #expect(lead.isLead)
        #expect(lead.stateTag == "作業中")
    }

    @Test("an idle participant has no tag, an away one says so, and the lead is the only role shown")
    func tags() throws {
        #expect(try #require(section.cards.first { $0.id == "frontend" }).presence == .waiting)
        #expect(Ja.People.stateTag(.idle) == nil)
        #expect(try #require(section.cards.first { $0.id == "docs" }).stateTag == "離席")
        #expect(!(try #require(section.cards.first { $0.id == "backend" }).isLead))
    }

    @Test("work in progress has its forecast, work that cannot move has its reason and since when")
    func workRows() throws {
        let doing = try #require(section.doing.first)
        #expect(doing.ref == "app#12")
        #expect(doing.assigneeLabel == "Backend")
        #expect(doing.etaText?.contains("ごろに終わる見込み、遅くとも") == true)
        #expect(doing.etaBasis == "8 finished items with 3 points")
        #expect(doing.blockedText == nil)
        #expect(section.blocked.map(\.ref) == ["app#14", "app#15"])
        #expect(section.blocked[0].blockedText == "判断待ち 11月23日 から")
        #expect(section.blocked[0].assigneeLabel == "担当なし")
        #expect(section.blocked[0].etaText == nil)
        #expect(section.blocked[1].blockedText == "先行する仕事待ち 11月24日 から")
        #expect(section.blocked[1].assigneeLabel == "Docs")
    }

    @Test("an assignee the team does not list shows as the name it was given")
    func unknownAssignee() throws {
        let item = try JSONDecoder().decode(WorkItem.self, from: Data(#"{"ref":"x#1","title":"t","status":"inProgress","assignee":"ghost"}"#.utf8))
        let row = WorkRow(item: item, view: Teams.team().view)
        #expect(row.assigneeLabel == "ghost")
        #expect(row.etaText == Ja.People.etaNone)
    }

    @Test("a forecast that is over budget says so with its basis")
    func overBudget() throws {
        let json = #"{"ref":"x#1","title":"t","status":"inProgress","eta":{"p50":"2026-11-25T03:00:00Z","p80":"2026-11-25T09:00:00Z","basis":"3 items","isOverBudget":true}}"#
        let row = WorkRow(item: try JSONDecoder().decode(WorkItem.self, from: Data(json.utf8)), view: Teams.team().view)
        #expect(row.etaText == "見込みを過ぎています (3 items)")
    }

    @Test("epics: the one in progress first, with its bar, forecast and what blocks it; one with no forecast says why")
    func epics() {
        #expect(section.epics.map(\.ref) == ["app#10", "app#20"])
        let first = section.epics[0]
        #expect(first.count == "1 / 4")
        #expect(first.barLabel == "4 件のうち、完了 1、進行中 1、停止 2")
        #expect(first.meta.hasSuffix("ごろに終わる見込み"))
        #expect(first.blockedBy == "停止: 判断待ち 1、先行する仕事待ち 1")
        let second = section.epics[1]
        #expect(second.meta == "No recent throughput to forecast from")
        #expect(second.blockedBy == nil)
        #expect(second.segments.isEmpty)
    }

    @Test("a long list shows its first rows and says how many are folded away")
    func folding() {
        let rows = Array(1...20)
        #expect(PeopleLimits.shown(rows, limit: PeopleLimits.rowsShown, isExpanded: false).items.count == 8)
        #expect(PeopleLimits.shown(rows, limit: PeopleLimits.rowsShown, isExpanded: false).hidden == 12)
        #expect(PeopleLimits.shown(rows, limit: PeopleLimits.heldShown, isExpanded: true).hidden == 0)
        #expect(PeopleLimits.shown([1, 2], limit: 6, isExpanded: false).hidden == 0)
    }

    @Test("a copy says how old it is in the desk's words, and a team on this node says nothing")
    func replica() {
        #expect(ReplicaNotice.text(for: Teams.team(), now: now) == nil)
        let stale: [String: Any] = ["kind": "replica", "fetchedAt": "2026-11-24T10:00:00Z", "triedAt": "2026-11-24T12:00:00Z", "error": "refused"]
        #expect(ReplicaNotice.text(for: Teams.team(source: stale), now: now) == Ja.Desk.staleCopy(when: "2 時間前", reason: "refused"))
        let never: [String: Any] = ["kind": "replica", "triedAt": "2026-11-24T12:00:00Z"]
        #expect(ReplicaNotice.text(for: Teams.team(source: never), now: now) == Ja.Desk.neverRead(reason: "—"))
    }
}
