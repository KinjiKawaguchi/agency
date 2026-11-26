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

    @Test("a team's participants drawn after its charts: the three-part bar, the words, a participant with no session, and the unattributed line")
    func participants() throws {
        func sample(from file: String = #filePath) throws -> TeamEvaluationReport {
            var url = URL(fileURLWithPath: file)
            for _ in 0..<5 { url.deleteLastPathComponent() }
            let data = try Data(contentsOf: url.appendingPathComponent("testdata/evaluation.sample.json"))
            let evaluation = try JSONDecoder().decode(Evaluation.self, from: data)
            return TeamEvaluationReport(numbers: evaluation.teams[0], label: UIFixtures.unbrokenID, days: evaluation.days)
        }
        let report = try sample()
        let section = try #require(report.participants)
        for scheme in [ColorScheme.light, .dark] {
            expectFits(Snapshot.capture("evaluation-participants", ParticipantsView(section: section), scheme: scheme), "participants, \(scheme)")
        }
        let ghost = #"{"team":"app","from":"2026-11-11","until":"2026-11-24","summary":{"merged":{"feat":1},"mainRedSeconds":0,"longestRedSeconds":0,"reverts":0},"byDay":[],"participants":[{"name":"\#(UIFixtures.unbrokenID)","busySeconds":0,"waitingSeconds":0,"idleSeconds":0,"costUsd":0.5,"tokens":{"input":1,"output":2,"cacheRead":3,"cacheWrite":4},"merged":0}]}"#
        let numbers = try JSONDecoder().decode(TeamEvaluation.self, from: Data(ghost.utf8))
        expectFits(Snapshot.capture("evaluation-participants-ghost", ParticipantsView(section: try #require(ParticipantsSection(numbers: numbers)))), "no session")
    }

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

    // MARK: Effort

    private func effortReport(plan: PlanState, label: String = "App", over: Bool = false) -> TeamEffortReport {
        let types = EffortType.allCases.enumerated().map { index, type in
            #""\#(type.rawValue)":{"costUsd":\#(index == 5 ? 3.5 : Double(10 - index) * 12.25),"tokens":{"input":1,"output":2,"cacheRead":3,"cacheWrite":4}}"#
        }.joined(separator: ",")
        let days = (1...30).map { day -> String in
            let costs = EffortType.allCases.enumerated().map { #""\#($1.rawValue)":\#(Double((day * 3 + $0 * 5) % 17) + (over ? 40 : 1))"# }.joined(separator: ",")
            return #"{"date":"\#(String(format: "2026-11-%02d", day))","costUsd":{\#(costs)}}"#
        }.joined(separator: ",")
        let json = #"{"team":"app","from":"2026-11-01","until":"2026-11-30","byType":{\#(types)},"byDay":[\#(days)]}"#
        return TeamEffortReport(effort: try! JSONDecoder().decode(TeamEffort.self, from: Data(json.utf8)), label: label, days: 30, plan: plan)
    }

    private var plan: TeamPlan {
        TeamPlan(allocation: [.newFeature: 50, .improvement: 20, .maintenance: 20, .debt: 10, .incident: 0], weeklyBudgetUsd: 900)
    }

    @Test("the effort of a team against a plan, over its budget, and of teams with no plan fit the screen in light and dark")
    func effort() {
        let state = EffortState.ready(
            [effortReport(plan: .stated(plan), label: UIFixtures.longJapaneseTitle, over: true), effortReport(plan: .none, label: "Free"), effortReport(plan: .unreadable, label: "Broken")],
            unavailable: [TeamUnavailableNote(team: "ops", label: "ops", reason: "the team's node could not be reached")]
        )
        let view = EffortSection(state: state, selectedDay: "2026-11-12")
        for scheme in [ColorScheme.light, .dark] {
            expectFits(Snapshot.capture("effort", view, scheme: scheme), "the effort (\(scheme))")
        }
    }

    @Test("the other outcomes of the effort fit the screen")
    func effortOutcomes() {
        let states: [EffortState] = [.loading, .unavailable, .couldNotAnswer("runtime busy"), .failed("connection refused")]
        let view = VStack(alignment: .leading, spacing: 24) { ForEach(Array(states.enumerated()), id: \.offset) { EffortSection(state: $1) } }
        expectFits(Snapshot.capture("effort-outcomes", view), "the other outcomes of the effort")
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
