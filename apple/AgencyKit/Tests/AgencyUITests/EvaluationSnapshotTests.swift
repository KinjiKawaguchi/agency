#if canImport(SwiftUI) && os(macOS)
import AgencyAPI
import AgencyModel
import SwiftUI
import Testing
@testable import AgencyUI

/// The evaluation drawn at phone width from made-up numbers. The rule checked is the owner's: nothing is
/// wider than the screen. With `AGENCY_SNAPSHOT_DIR` set, each test leaves a PNG to look at.
@MainActor
@Suite("Evaluation renderings", .serialized)
struct EvaluationSnapshotTests {
    private let width = Snapshot.phoneWidth

    private func expectFits(_ result: Snapshot.Result, _ name: String, sourceLocation: SourceLocation = #_sourceLocation) {
        #expect(!result.png.isEmpty, "\(name) was not drawn", sourceLocation: sourceLocation)
        #expect(result.size.width == width, "\(name) is \(result.size.width) points wide, the screen is \(width)", sourceLocation: sourceLocation)
        #expect(result.size.height > 40, sourceLocation: sourceLocation)
    }

    // MARK: Made-up numbers

    private func date(_ offset: Int) -> String { String(format: "2026-11-%02d", offset) }

    /// A month of a team that ships a little every day, with a long name.
    private func report(days: Int, label: String = "App") -> TeamEvaluationReport {
        let rows = (0..<days).map { index -> String in
            let day = index + 1
            let lead = (0..<(index % 4)).map { "\(1_200 + ($0 + index) * 1_700)" }.joined(separator: ",")
            return #"{"date":"\#(String(format: "2026-11-%02d", day))","merged":{"feat":\#(index % 3),"fix":\#(index % 2),"refactor":\#(index % 5 == 0 ? 1 : 0),"docs":\#(index % 4 == 0 ? 1 : 0),"test":0,"chore":\#(index % 3 == 0 ? 2 : 0),"other":0},"reverts":\#(index % 9 == 0 ? 1 : 0),"mainRedSeconds":\#(index % 6 == 0 ? 2_700 + index * 90 : 0),"costUsd":\#(8.5 + Double(index % 7)),"prLeadSeconds":[\#(lead)],"issueLeadSeconds":[\#(index % 5 == 0 ? "20000" : "")]}"#
        }.joined(separator: ",")
        let json = #"{"team":"app","from":"2026-11-01","until":"\#(String(format: "2026-11-%02d", days))","summary":{"merged":{"feat":30,"fix":15,"refactor":6,"docs":8,"test":0,"chore":20,"other":1},"prLeadMedianSeconds":5400,"prLeadP90Seconds":86400,"issueLeadMedianSeconds":172800,"issueLeadP90Seconds":432000,"mainRedSeconds":37800,"longestRedSeconds":9000,"reverts":4,"costPerMergedPrUsd":12.34},"byDay":[\#(rows)]}"#
        let numbers = try! JSONDecoder().decode(TeamEvaluation.self, from: Data(json.utf8))
        return TeamEvaluationReport(numbers: numbers, label: label, days: days)
    }

    private func orgRef(_ id: String, _ label: String) -> OrgRef {
        try! JSONDecoder().decode(OrgRef.self, from: Data(#"{"id":"\#(id)","label":"\#(label)","isRoot":true,"teams":[]}"#.utf8))
    }

    private var orgs: [OrgRef] { [orgRef("acme", "Acme"), orgRef("zed", UIFixtures.unbrokenID)] }

    // MARK: Renderings

    @Test("a month of a team with a long name, and a team that could not answer, fit the screen in light and dark")
    func month() {
        let state = EvaluationState.ready(
            [report(days: 30, label: UIFixtures.longJapaneseTitle)],
            unavailable: [TeamUnavailableNote(team: "ops", label: "ops", reason: "the team's node could not be reached")]
        )
        let view = EvaluationContent(orgs: orgs, org: orgs[0], days: 30, state: state, selectedDay: "2026-11-12")
        for scheme in [ColorScheme.light, .dark] {
            expectFits(Snapshot.capture("evaluation", view, scheme: scheme), "the evaluation (\(scheme))")
        }
    }

    @Test("two weeks of one team")
    func fortnight() {
        let view = EvaluationContent(orgs: [orgs[0]], org: orgs[0], days: 14, state: .ready([report(days: 14)], unavailable: []))
        expectFits(Snapshot.capture("evaluation-14", view), "two weeks")
    }

    @Test("the other outcomes fit the screen: nothing served yet, could not answer, loading, no teams, no orgs")
    func outcomes() {
        let states: [EvaluationState] = [.unavailable, .couldNotAnswer("runtime busy"), .failed("connection refused"), .loading, .ready([], unavailable: [])]
        let view = VStack(alignment: .leading, spacing: 24) {
            ForEach(Array(states.enumerated()), id: \.offset) { _, state in
                EvaluationContent(orgs: [orgs[0]], org: orgs[0], days: 14, state: state)
            }
            EvaluationContent(orgs: [], org: nil, days: 14, state: .loading)
        }
        expectFits(Snapshot.capture("evaluation-outcomes", view), "the other outcomes")
    }

    @Test("a team with no pull request merged has empty charts and says so")
    func empty() {
        let json = #"{"team":"app","from":"2026-11-11","until":"2026-11-24","summary":{"merged":{},"mainRedSeconds":0,"longestRedSeconds":0,"reverts":0},"byDay":[]}"#
        let numbers = try! JSONDecoder().decode(TeamEvaluation.self, from: Data(json.utf8))
        let view = TeamEvaluationView(report: TeamEvaluationReport(numbers: numbers, label: "Quiet", days: 14))
        expectFits(Snapshot.capture("evaluation-empty", view), "a team with nothing")
    }
}
#endif
