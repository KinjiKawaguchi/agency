@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

@Suite("Funding summary and wording")
struct FundingSummaryTests {
    private let now = Date(timeIntervalSince1970: 1_795_521_600)

    private func funding(_ windows: String, readAt: String? = "2026-11-24T11:00:00Z") -> Funding {
        let read = readAt.map { #","readAt":"\#($0)""# } ?? ""
        let json = #"{"id":"sub","label":"Team plan","kind":"subscription","windows":[\#(windows)]\#(read)}"#
        return try! JSONDecoder().decode(Funding.self, from: Data(json.utf8))
    }

    @Test("lists the weekly window first and projects each one")
    func weeklyFirst() {
        let five = #"{"name":"fiveHour","lengthHours":5,"utilization":40,"resetsAt":"2026-11-24T14:00:00Z"}"#
        let seven = #"{"name":"sevenDay","lengthHours":168,"utilization":10,"resetsAt":"2026-11-27T12:00:00Z"}"#
        let summary = FundingSummary(funding: funding("\(five),\(seven)"), now: now)
        #expect(summary.windows.map(\.name) == [.sevenDay, .fiveHour])
        for window in summary.windows {
            guard case .projected = window else { Issue.record("expected a projection for \(window.name)"); return }
        }
    }

    @Test("a window the node has no reading of is not started, and one read before its reset has no forecast")
    func notStartedAndStale() {
        let old = #"{"name":"fiveHour","lengthHours":5,"utilization":40,"resetsAt":"2026-11-24T10:00:00Z"}"#
        let summary = FundingSummary(funding: funding(old), now: now)
        #expect(summary.windows == [.notStarted(.sevenDay), .stale(.fiveHour)])
    }

    @Test("a source with no windows says its usage is unknown, not that the windows have not started")
    func unknown() {
        let summary = FundingSummary(funding: funding("", readAt: nil), now: now)
        #expect(summary.windows.isEmpty)
        #expect(summary.readAt == nil)
    }

    @Test("money, percent and dates read as the web client writes them")
    func format() throws {
        #expect(JaFormat.money(0.5) == "$0.50")
        #expect(JaFormat.money(9.994) == "$9.99")
        #expect(JaFormat.money(12.4) == "$12")
        #expect(JaFormat.money(1234.5) == "$1,235")
        #expect(JaFormat.money(1_234_567) == "$1,234,567")
        #expect(JaFormat.percent(61.5) == "62%")
        let utc = try #require(TimeZone(identifier: "UTC"))
        #expect(JaFormat.dateTime(now, in: utc) == "11月24日 12:00")
    }

    @Test("the overview's words are the web client's, and the glossary's: 参加者, not 人, for the people who work")
    func words() {
        #expect(Ja.Overview.lead(requests: 0, teams: 0) == "あなたを待っている依頼は、どのチームにもありません。")
        #expect(Ja.Overview.lead(requests: 3, teams: 2) == "2 チームで、3 件の依頼があなたを待っています。")
        #expect(Ja.Overview.working(1, of: 4) == "参加者 4 人のうち 1 人が作業中")
        #expect(Ja.Overview.tag(.active) == nil)
        #expect(Ja.Overview.tag(.mine) == "あなたの番")
        #expect(Ja.Work.blockedReason(.waitingOnPrevious) == "先行する仕事待ち")
    }
}

@Suite("Data age")
struct DataAgeTests {
    private let now = Date(timeIntervalSince1970: 1_795_521_600)

    @Test("a team on this node has no age to show")
    func local() {
        #expect(DataAge(team: OverviewTeams.team(), now: now) == nil)
    }

    @Test("a copy says how old it is, and a copy that could not be renewed says why")
    func replica() {
        let fresh: [String: Any] = ["kind": "replica", "fetchedAt": "2026-11-24T11:00:00Z", "triedAt": "2026-11-24T11:00:00Z"]
        #expect(DataAge(team: OverviewTeams.team(source: fresh), now: now) == DataAge(text: "1 時間前 のデータ", isStale: false))
        let stale: [String: Any] = ["kind": "replica", "fetchedAt": "2026-11-24T10:00:00Z", "triedAt": "2026-11-24T12:00:00Z", "error": "refused"]
        #expect(DataAge(team: OverviewTeams.team(source: stale), now: now)?.isStale == true)
        #expect(DataAge(team: OverviewTeams.team(source: stale), now: now)?.text == "2 時間前 のデータです。チームのノードに接続できません: refused")
    }

    @Test("a copy never read says so")
    func neverRead() {
        let never: [String: Any] = ["kind": "replica", "triedAt": "2026-11-24T12:00:00Z", "error": "refused"]
        #expect(DataAge(team: OverviewTeams.team(source: never), now: now)?.text == "まだ読めていません: refused")
    }
}
