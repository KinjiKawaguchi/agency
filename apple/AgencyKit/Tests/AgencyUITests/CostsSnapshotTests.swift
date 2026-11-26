#if canImport(SwiftUI) && os(macOS)
import AgencyAPI
import AgencyModel
import SwiftUI
import Testing
@testable import AgencyUI

/// The costs screen drawn at phone width from made-up usage. The rule checked is the owner's: nothing is
/// wider than the screen. With `AGENCY_SNAPSHOT_DIR` set, each test leaves a PNG to look at.
@MainActor
@Suite("Costs renderings", .serialized)
struct CostsSnapshotTests {
    private let now = UIFixtures.now
    private let width = Snapshot.phoneWidth

    private func expectFits(_ result: Snapshot.Result, _ name: String, sourceLocation: SourceLocation = #_sourceLocation) {
        #expect(!result.png.isEmpty, "\(name) was not drawn", sourceLocation: sourceLocation)
        #expect(result.size.width == width, "\(name) is \(result.size.width) points wide, the screen is \(width)", sourceLocation: sourceLocation)
        #expect(result.size.height > 40, sourceLocation: sourceLocation)
    }

    // MARK: Made-up usage

    private func usage(days: Int, people: [String]) -> TokenUsage {
        let byDay = (0..<days).map { day -> String in
            let costs = people.enumerated().map { index, person in
                #""\#(person)":\#(Double((day * 7 + index * 3) % 11) + 0.5)"#
            }.joined(separator: ",")
            return #"{"date":"2026-11-\#(String(format: "%02d", day + 1))","costUsd":{\#(costs)}}"#
        }.joined(separator: ",")
        let byPerson = people.enumerated().map { index, person in
            #"{"person":"\#(person)","costUsd":\#(Double(index + 1) * 12.34),"tokens":{"input":\#(120_000 * (index + 1)),"output":\#(45_000 * (index + 1)),"cacheRead":\#(2_400_000 * (index + 1)),"cacheWrite":18000}}"#
        }.joined(separator: ",")
        let json = #"{"org":"acme","team":"app","days":\#(days),"generatedAt":"2026-11-24T12:00:00Z","byDay":[\#(byDay)],"byPerson":[\#(byPerson)]}"#
        return try! JSONDecoder().decode(TokenUsage.self, from: Data(json.utf8))
    }

    private func costsTeam(_ usage: UsageState, label: String = "App", id: String = "app") -> CostsTeam {
        let people = ["lead", "backend", "frontend", "docs"].map { id in
            #"{"id":"\#(id)","label":"\#(id.capitalized)","kind":"agent","role":"member","state":"idle","status":"","holding":[]}"#
        }.joined(separator: ",")
        let view = #"{"org":"acme","team":"\#(id)","label":"\#(label)","generatedAt":"2026-11-24T12:00:00Z","people":[\#(people)],"requests":[],"work":[],"epics":[]}"#
        let snapshot = #"{"view":\#(view),"viewer":"owner","mine":[],"waitingOnViewer":[],"source":{"kind":"local"}}"#
        return CostsTeam(team: Team(snapshot: try! JSONDecoder().decode(TeamSnapshot.self, from: Data(snapshot.utf8))), usage: usage)
    }

    private func funding(_ json: String) -> FundingSummary {
        FundingSummary(funding: try! JSONDecoder().decode(Funding.self, from: Data(json.utf8)), now: now)
    }

    private var sources: [FundingSummary] {
        [
            funding(#"{"id":"s1","label":"Team plan","kind":"subscription","readAt":"2026-11-24T14:50:00Z","windows":["#
                + #"{"name":"fiveHour","lengthHours":5,"utilization":74,"resetsAt":"2026-11-24T17:00:00Z"},"#
                + #"{"name":"sevenDay","lengthHours":168,"utilization":30,"resetsAt":"2026-11-28T12:00:00Z"}]}"#),
            funding(#"{"id":"s2","label":"\#(UIFixtures.unbrokenID)","kind":"apiKey","windows":[]}"#),
        ]
    }

    // MARK: Renderings

    @Test("a month of usage by many participants, with a long name, fits the screen in light and dark")
    func busy() {
        let people = ["lead", "backend", "frontend", "docs", UIFixtures.unbrokenID, UIFixtures.longJapaneseTitle, "p6", "p7", "p8", "p9", "p10"]
        let teams = [
            costsTeam(.ready(usage(days: 30, people: people))),
            costsTeam(.ready(usage(days: 7, people: ["lead", "backend"])), label: "Site", id: "site"),
        ]
        let view = CostsContent(funding: sources, teams: teams, days: 30, now: now, selectedDay: "2026-11-12")
        for scheme in [ColorScheme.light, .dark] {
            expectFits(Snapshot.capture("costs", view, scheme: scheme), "the costs screen (\(scheme))")
        }
    }

    @Test("a team that reports nothing, one that could not answer, one that failed and one still loading fit the screen")
    func outcomes() {
        let teams = [
            costsTeam(.unavailable, label: "Quiet", id: "q"), costsTeam(.couldNotAnswer("runtime busy"), label: "Busy", id: "b"),
            costsTeam(.failed("connection refused"), label: "Down", id: "d"), costsTeam(.loading, label: "Slow", id: "s"),
        ]
        let view = CostsContent(funding: [], teams: teams, days: 14, now: now)
        expectFits(Snapshot.capture("costs-outcomes", view), "the other outcomes")
    }

    @Test("a day with no cost at all does not break the chart")
    func emptyDays() {
        let report = costsTeam(.ready(usage(days: 1, people: [])))
        expectFits(Snapshot.capture("costs-none", TeamTokens(team: report)), "usage with nothing in it")
    }
}
#endif
