import Foundation
import Testing
@testable import AgencyAPI

/// The shared sample (`testdata/evaluation.sample.json`) is the contract the node's route and the clients share.
@Suite("Evaluation wire types")
struct EvaluationWireTests {
    private func sample(from file: String = #filePath) throws -> Data {
        // Tests/AgencyAPITests/EvaluationWireTests.swift: five levels up is the repository root.
        var url = URL(fileURLWithPath: file)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        return try Data(contentsOf: url.appendingPathComponent("testdata/evaluation.sample.json"))
    }

    private func decode(_ json: String) throws -> Evaluation {
        try JSONDecoder().decode(Evaluation.self, from: Data(json.utf8))
    }

    private func answer(team: String) -> String {
        #"{"org":"acme","days":14,"from":"2026-11-11","until":"2026-11-24","generatedAt":"2026-11-24T12:00:00Z","teams":[\#(team)],"unavailable":[]}"#
    }

    @Test("the shared sample decodes: the period, a team's summary and days, a team with nothing, a team that could not answer")
    func shared() throws {
        let evaluation = try JSONDecoder().decode(Evaluation.self, from: try sample())
        #expect(evaluation.org == "acme")
        #expect(evaluation.days == 14)
        #expect(evaluation.from == "2026-11-11")
        #expect(evaluation.until == "2026-11-24")
        #expect(evaluation.teams.map(\.team) == ["app", "docs"])
        let app = evaluation.teams[0]
        #expect(app.summary.merged == MergedCounts(feat: 3, fix: 1, docs: 1, chore: 1))
        #expect(app.summary.merged.total == 6)
        #expect(app.summary.prLeadMedianSeconds == 5400)
        #expect(app.summary.prLeadP90Seconds == 14400)
        #expect(app.summary.issueLeadMedianSeconds == 15300)
        #expect(app.summary.reverts == 1)
        #expect(app.summary.costPerMergedPrUsd == 1.9)
        #expect(app.from == "2026-11-11")
        #expect(app.until == "2026-11-24")
        #expect(app.byDay.map(\.date) == ["2026-11-23", "2026-11-24"])
        #expect(app.byDay[0].prLeadSeconds == [3600, 5400, 14400])
        #expect(app.byDay[1].issueLeadSeconds.isEmpty)
        #expect(app.byDay[1].reverts == 1)
        let docs = evaluation.teams[1]
        #expect(docs.summary.prLeadMedianSeconds == nil)
        #expect(docs.summary.costPerMergedPrUsd == nil)
        #expect(docs.byDay.isEmpty)
        #expect(evaluation.unavailable == [TeamUnavailable(team: "ops", reason: "the team's node could not be reached")])
    }

    @Test("a type the node leaves out counts as 0, and a list that is missing is empty")
    func missing() throws {
        let evaluation = try decode(answer(team: #"{"team":"app","from":"2026-11-11","until":"2026-11-24","summary":{"merged":{"feat":2},"mainRedSeconds":0,"longestRedSeconds":0,"reverts":0},"byDay":[{"date":"2026-11-24"}]}"#))
        let team = evaluation.teams[0]
        #expect(team.summary.merged == MergedCounts(feat: 2))
        #expect(team.byDay[0].merged.total == 0)
        #expect(team.byDay[0].prLeadSeconds.isEmpty)
        #expect(team.byDay[0].costUsd == 0)
    }

    @Test("a number that is negative refuses the whole answer, wherever it is")
    func negative() {
        let summary = #""summary":{"merged":{"feat":1},"mainRedSeconds":0,"longestRedSeconds":0,"reverts":0}"#
        #expect(throws: DecodingError.self) { try decode(answer(team: #"{"team":"a","from":"2026-11-11","until":"2026-11-24",\#(summary.replacingOccurrences(of: #""feat":1"#, with: #""feat":-1"#)),"byDay":[]}"#)) }
        #expect(throws: DecodingError.self) { try decode(answer(team: #"{"team":"a","from":"2026-11-11","until":"2026-11-24",\#(summary.replacingOccurrences(of: #""reverts":0"#, with: #""reverts":-2"#)),"byDay":[]}"#)) }
        #expect(throws: DecodingError.self) { try decode(answer(team: #"{"team":"a","from":"2026-11-11","until":"2026-11-24",\#(summary),"byDay":[{"date":"2026-11-24","prLeadSeconds":[60,-1]}]}"#)) }
        #expect(throws: DecodingError.self) { try decode(answer(team: #"{"team":"a","from":"2026-11-11","until":"2026-11-24",\#(summary),"byDay":[{"date":"2026-11-24","costUsd":-0.5}]}"#)) }
    }

    @Test("a count that is not a whole number, and a value that is not a number, refuse the answer")
    func notWhole() {
        let summary = #""summary":{"merged":{"feat":1.5},"mainRedSeconds":0,"longestRedSeconds":0,"reverts":0}"#
        #expect(throws: DecodingError.self) { try decode(answer(team: #"{"team":"a","from":"2026-11-11","until":"2026-11-24",\#(summary),"byDay":[]}"#)) }
        let text = #""summary":{"merged":{"feat":"many"},"mainRedSeconds":0,"longestRedSeconds":0,"reverts":0}"#
        #expect(throws: DecodingError.self) { try decode(answer(team: #"{"team":"a","from":"2026-11-11","until":"2026-11-24",\#(text),"byDay":[]}"#)) }
    }

    @Test("a team has its own period: it names the first and last day in its node's time zone")
    func teamPeriod() {
        let summary = #""summary":{"merged":{},"mainRedSeconds":0,"longestRedSeconds":0,"reverts":0}"#
        #expect(throws: DecodingError.self) { try decode(answer(team: #"{"team":"a",\#(summary),"byDay":[]}"#)) }
        #expect(throws: DecodingError.self) { try decode(answer(team: #"{"team":"a","from":"2026-11-11",\#(summary),"byDay":[]}"#)) }
    }

    @Test("the period and the list of teams that could not answer are always there")
    func required() {
        #expect(throws: DecodingError.self) { try decode(#"{"org":"acme","days":14,"generatedAt":"2026-11-24T12:00:00Z","teams":[],"unavailable":[]}"#) }
        #expect(throws: DecodingError.self) { try decode(#"{"org":"acme","days":14,"from":"2026-11-11","until":"2026-11-24","generatedAt":"2026-11-24T12:00:00Z","teams":[]}"#) }
    }
}
