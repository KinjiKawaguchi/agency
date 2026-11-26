@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

@Suite("Participants section")
struct ParticipantsSectionTests {
    private func owner() throws -> TeamEvaluation { try TeamEvaluationReportTests.sample().teams[0] }

    private func team(_ extra: String) throws -> TeamEvaluation {
        let json = #"{"team":"app","from":"2026-11-11","until":"2026-11-24","summary":{"merged":{"feat":1},"mainRedSeconds":0,"longestRedSeconds":0,"reverts":0},"byDay":[]\#(extra)}"#
        return try JSONDecoder().decode(TeamEvaluation.self, from: Data(json.utf8))
    }

    private func participant(_ name: String, busy: Int = 0, waiting: Int = 0, idle: Int = 0, merged: Int = 0, perPr: String = "") -> String {
        #"{"name":"\#(name)","busySeconds":\#(busy),"waitingSeconds":\#(waiting),"idleSeconds":\#(idle),"costUsd":2.5,"tokens":{"input":1,"output":2,"cacheRead":3,"cacheWrite":4},"merged":\#(merged)\#(perPr)}"#
    }

    @Test("the owner's reading has a row for each participant, with the time in three parts, the cost, the tokens and the merged")
    func rows() throws {
        let section = try #require(ParticipantsSection(numbers: try owner()))
        #expect(section.rows.map(\.name) == ["backend", "lead", "reviewer"])
        let backend = section.rows[0]
        #expect(backend.times.map(\.label) == ["作業", "判断待ち（下限）", "待機"])
        #expect(backend.times.map(\.duration) == [DurationFormat.text(30_600), DurationFormat.text(2_400), DurationFormat.text(4_800)])
        let total = 30_600.0 + 2_400 + 4_800
        #expect(backend.times.map(\.share) == [30_600 / total, 2_400 / total, 4_800 / total])
        #expect(abs(backend.times.map(\.share).reduce(0, +) - 1) < 1e-9)
        #expect(backend.cost == "$8.45")
        #expect(backend.tokens == "2,745,000 トークン", "all four kinds added up, said as tokens")
        #expect(backend.merged == "3" && backend.mergedSpoken == "マージしたプルリクエスト 3 件")
        #expect(backend.costPerPullRequest == JaFormat.money(try #require(try owner().participants?[0].costPerMergedPrUsd)))
        #expect(backend.timesSpoken.hasPrefix("作業 ") && backend.timesSpoken.contains("判断待ち ") && backend.timesSpoken.contains("以上"))
    }

    @Test("a participant who merged nothing says so: never a $0 cost per pull request")
    func nothingMerged() throws {
        let reviewer = try #require(ParticipantsSection(numbers: try owner())).rows[2]
        #expect(reviewer.merged == "0")
        #expect(reviewer.costPerPullRequest == "マージなし")
    }

    @Test("nothing is drawn when the node gave no participants, and nothing when it gave an empty list, as on the web")
    func noSection() throws {
        let docs = try TeamEvaluationReportTests.sample().teams[1]
        #expect(ParticipantsSection(numbers: docs) == nil)
        #expect(ParticipantsSection(numbers: try team(#","participants":[]"#)) == nil)
        #expect(TeamEvaluationReport(numbers: docs, label: "Docs", days: 14).participants == nil)
        #expect(TeamEvaluationReport(numbers: try owner(), label: "App", days: 14).participants != nil)
    }

    @Test("the pull requests no participant is named the author of are said only when there are some, and add up with the participants")
    func unattributed() throws {
        let sample = try #require(ParticipantsSection(numbers: try owner()))
        #expect(sample.unattributed?.hasPrefix("参加者が分からないマージ: 1 件。") == true)
        let none = try team(#","participants":[\#(participant("a", busy: 10))],"unattributedMerged":0"#)
        #expect(ParticipantsSection(numbers: none)?.unattributed == nil, "a 0 is not said")
        let absent = try team(#","participants":[\#(participant("a", busy: 10))]"#)
        #expect(ParticipantsSection(numbers: absent)?.unattributed == nil)
        let many = try team(#","participants":[\#(participant("a", busy: 10))],"unattributedMerged":4"#)
        #expect(ParticipantsSection(numbers: many)?.unattributed?.contains("4 件") == true)
    }

    @Test("a participant with no session alive has no bar, only the words that say so")
    func noSession() throws {
        let row = try #require(ParticipantsSection(numbers: try team(#","participants":[\#(participant("ghost", merged: 2, perPr: #","costPerMergedPrUsd":1.25"#))]"#))).rows[0]
        #expect(row.times.isEmpty && !row.hasSession)
        #expect(row.merged == "2" && row.costPerPullRequest == "$1.25")
    }

    @Test("a participant whose time is all in one part has one full part and two empty ones")
    func onePart() throws {
        let row = try #require(ParticipantsSection(numbers: try team(#","participants":[\#(participant("a", waiting: 600))]"#))).rows[0]
        #expect(row.times.map(\.share) == [0, 1, 0])
    }

    @Test("the words are the web's")
    func words() {
        #expect(Ja.Evaluation.Participants.title == "参加者")
        #expect(Ja.Evaluation.Participants.timeNote.contains("重なったセッションは 1 回として数えます"))
        #expect(Ja.Evaluation.Participants.timeNote.contains("下限"))
        #expect(Ja.Evaluation.Participants.noSession == "この期間に生きていたセッションはありません。")
        #expect(Ja.Evaluation.Participants.perPullRequest == "マージしたプルリクエスト 1 件あたりのコスト")
    }
}
