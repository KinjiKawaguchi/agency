import Foundation
import Testing
@testable import AgencyAPI

/// The participants of a team's evaluation (agency#364): there only for the team's owner, so every reading of them starts from
/// "the node may not have said".
@Suite("Evaluation participants wire")
struct EvaluationParticipantsWireTests {
    private func data(_ path: String, from file: String = #filePath) throws -> Data {
        var url = URL(fileURLWithPath: file)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        return try Data(contentsOf: url.appendingPathComponent(path))
    }

    private func team(_ extra: String) throws -> TeamEvaluation {
        let json = #"{"team":"app","from":"2026-11-11","until":"2026-11-24","summary":{"merged":{"feat":1},"mainRedSeconds":0,"longestRedSeconds":0,"reverts":0},"byDay":[]\#(extra)}"#
        return try JSONDecoder().decode(TeamEvaluation.self, from: Data(json.utf8))
    }

    @Test("the shared sample and the node's own answer decode, the participants add up with the unattributed to the summary's pull requests")
    func samples() throws {
        for path in ["testdata/evaluation.sample.json", "testdata/client/evaluation.json"] {
            let evaluation = try JSONDecoder().decode(Evaluation.self, from: try data(path))
            let app = try #require(evaluation.teams.first { $0.team == "app" })
            let participants = try #require(app.participants, Comment(rawValue: path))
            #expect(participants.map(\.name) == ["backend", "lead", "reviewer"], Comment(rawValue: path))
            #expect(participants.map(\.merged).reduce(0, +) + (app.unattributedMerged ?? -1) == app.summary.merged.total + app.summary.reverts, Comment(rawValue: path))
            #expect(participants[0].busySeconds == 30_600 && participants[0].waitingSeconds == 2_400 && participants[0].idleSeconds == 4_800)
            let backend = participants[0]
            let expected = (backend.costUsd / Double(backend.merged) * 100).rounded() / 100
            #expect(backend.costPerMergedPrUsd.map { abs($0 - expected) < 0.011 } == true, "cost over merged, to the cent")
            #expect(participants[2].costPerMergedPrUsd == nil, "none merged: no cost per pull request")
            #expect(participants[0].tokens.output == 45_000)
        }
    }

    @Test("a team read by someone who is not its owner has none: nil, which is not an empty list")
    func absent() throws {
        let evaluation = try JSONDecoder().decode(Evaluation.self, from: try data("testdata/evaluation.sample.json"))
        let docs = try #require(evaluation.teams.first { $0.team == "docs" })
        #expect(docs.participants == nil && docs.unattributedMerged == nil)
        #expect(try team("").participants == nil)
        #expect(try team(#","participants":[]"#).participants == [])
    }

    @Test("a negative or not-a-number time or cost refuses the answer, as every number of the evaluation does")
    func refusesBadNumbers() {
        let ok = #""name":"a","busySeconds":1,"waitingSeconds":0,"idleSeconds":0,"costUsd":1,"tokens":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"merged":0"#
        #expect(throws: DecodingError.self) { try team(#","participants":[{\#(ok.replacingOccurrences(of: "\"busySeconds\":1", with: "\"busySeconds\":-1"))}]"#) }
        #expect(throws: DecodingError.self) { try team(#","participants":[{\#(ok.replacingOccurrences(of: "\"costUsd\":1", with: "\"costUsd\":-0.5"))}]"#) }
        #expect(throws: DecodingError.self) { try team(#","participants":[{\#(ok.replacingOccurrences(of: "\"merged\":0", with: "\"merged\":-2"))}]"#) }
        #expect(throws: DecodingError.self) { try team(#","participants":[{"busySeconds":1}]"#) }
    }
}
