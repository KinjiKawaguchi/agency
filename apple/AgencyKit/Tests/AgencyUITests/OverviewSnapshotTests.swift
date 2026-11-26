#if canImport(SwiftUI) && os(macOS)
import AgencyAPI
import AgencyModel
import AgencyProtocol
import SwiftUI
import Testing
@testable import AgencyUI

/// The overview drawn at phone width, from made-up teams. The rule checked is the owner's: nothing is
/// wider than the screen. With `AGENCY_SNAPSHOT_DIR` set, each test leaves a PNG to look at.
@MainActor
@Suite("Overview renderings", .serialized)
struct OverviewSnapshotTests {
    private let now = UIFixtures.now
    private let width = Snapshot.phoneWidth

    private func expectFits(_ result: Snapshot.Result, _ name: String, sourceLocation: SourceLocation = #_sourceLocation) {
        #expect(!result.png.isEmpty, "\(name) was not drawn", sourceLocation: sourceLocation)
        #expect(result.size.width == width, "\(name) is \(result.size.width) points wide, the screen is \(width)", sourceLocation: sourceLocation)
        #expect(result.size.height > 40, sourceLocation: sourceLocation)
    }

    // MARK: Made-up teams

    private func person(_ id: String, _ state: String, status: String = "") -> [String: Any] {
        ["id": id, "label": id, "kind": "agent", "role": "member", "state": state, "status": status, "holding": [String]()]
    }

    private func blocked(_ ref: String, _ reason: String) -> [String: Any] {
        ["ref": ref, "title": ref, "status": "next", "blocked": ["reason": reason, "since": "2026-11-20"]]
    }

    private func team(
        _ id: String, people: [[String: Any]] = [], work: [[String: Any]] = [], mine: [String] = [],
        source: [String: Any] = ["kind": "local"]
    ) -> Team {
        let view: [String: Any] = [
            "org": "acme", "team": id, "label": id, "generatedAt": "2026-11-24T12:00:00Z",
            "people": people, "requests": [[String: Any]](), "work": work, "epics": [[String: Any]](),
        ]
        let body: [String: Any] = ["view": view, "viewer": "owner", "mine": mine, "waitingOnViewer": [String](), "source": source]
        let data = try! JSONSerialization.data(withJSONObject: body)
        return Team(snapshot: try! JSONDecoder().decode(TeamSnapshot.self, from: data))
    }

    private func entry(_ id: String, _ label: String, _ state: TeamState, org: String = "acme") -> TeamEntry {
        let ref = try! JSONDecoder().decode(TeamRef.self, from: Data(#"{"id":"\#(id)","label":"\#(label)","location":"local","viewer":"owner"}"#.utf8))
        return TeamEntry(org: org, ref: ref, state: state)
    }

    private func orgRef(_ id: String, _ label: String) -> OrgRef {
        try! JSONDecoder().decode(OrgRef.self, from: Data(#"{"id":"\#(id)","label":"\#(label)","isRoot":true,"teams":[]}"#.utf8))
    }

    private func funding(_ json: String) -> FundingSummary {
        FundingSummary(funding: try! JSONDecoder().decode(Funding.self, from: Data(json.utf8)), now: now)
    }

    private var teams: [TeamEntry] {
        let stale: [String: Any] = ["kind": "replica", "fetchedAt": "2026-11-24T10:00:00Z", "triedAt": "2026-11-24T14:55:00Z", "error": "connection refused"]
        return OverviewOrder.teams([
            entry("app", "app", .ready(team("app", people: [person("lead", "working"), person("backend", "working"), person("docs", "idle")], mine: ["r-1", "r-2"]))),
            entry("long", UIFixtures.longJapaneseTitle, .ready(team("long", people: [person("a", "working")], work: [
                blocked("x#1", "needsDecision"), blocked("x#2", "needsDecision"), blocked("x#3", "waitingOnPrevious"),
            ]))),
            entry("unbroken", UIFixtures.unbrokenID, .ready(team("unbroken", people: [person("a", "idle", status: "Nothing in Next"), person("b", "away", status: UIFixtures.unbrokenID)]))),
            entry("quiet", "docs", .ready(team("quiet", people: [person("a", "working")]))),
            entry("remote", "ops", .ready(team("remote", people: [person("a", "working")], source: stale))),
            entry("down", "billing", .failed(reason: "connection refused", signInEnded: false)),
            entry("ended", "legal", .failed(reason: "", signInEnded: true)),
            entry("slow", "research", .loading),
        ])
    }

    private func costs(_ entry: TeamEntry) -> WeekCost {
        switch entry.ref.id {
        case "app": .known(13.3)
        case "long": .known(1234.5)
        case "quiet": .none
        case "unbroken": .failed
        default: .loading
        }
    }

    private var fundingSources: [FundingSummary] {
        [
            funding(#"{"id":"s1","label":"Team plan","kind":"subscription","readAt":"2026-11-24T14:50:00Z","windows":["#
                + #"{"name":"fiveHour","lengthHours":5,"utilization":74,"resetsAt":"2026-11-24T17:00:00Z"},"#
                + #"{"name":"sevenDay","lengthHours":168,"utilization":30,"resetsAt":"2026-11-28T12:00:00Z"}]}"#),
            funding(#"{"id":"s2","label":"\#(UIFixtures.unbrokenID)","kind":"apiKey","windows":[]}"#),
            funding(#"{"id":"s3","label":"Old reading","kind":"subscription","windows":[{"name":"fiveHour","lengthHours":5,"utilization":40,"resetsAt":"2026-11-24T10:00:00Z"}]}"#),
        ]
    }

    private func content(sections: [OverviewStore.OrgSection]? = nil, funding: [FundingSummary]? = nil) -> OverviewContent {
        let entries = teams
        let shown = sections ?? [
            .init(org: orgRef("acme", "Acme"), teams: entries),
            .init(org: orgRef("empty", "Empty org"), teams: []),
        ]
        return OverviewContent(
            sections: shown, waiting: (2, 1), funding: funding ?? fundingSources, cost: costs, now: now
        )
    }

    // MARK: Renderings

    @Test("every kind of team card, and the funding sources, fit the screen in light and dark")
    func everything() {
        for scheme in [ColorScheme.light, .dark] {
            expectFits(Snapshot.capture("overview", content(), scheme: scheme), "the overview (\(scheme))")
        }
    }

    @Test("a team that needs the viewer, alone")
    func mineCard() {
        let card = TeamCard(entry: teams[0], cost: .known(13.3), now: now)
        expectFits(Snapshot.capture("overview-team-mine", card), "a team waiting on the viewer")
    }

    @Test("an overview with no funding and no teams says so and still fits")
    func empty() {
        let view = OverviewContent(sections: [.init(org: orgRef("e", "Empty org"), teams: [])], waiting: (0, 0), funding: [], cost: { _ in .loading }, now: now)
        expectFits(Snapshot.capture("overview-empty", view), "the empty overview")
    }

    @Test("a funding source with both windows, in light and dark")
    func fundingCard() {
        for scheme in [ColorScheme.light, .dark] {
            expectFits(Snapshot.capture("overview-funding", FundingCard(source: fundingSources[0], now: now), scheme: scheme), "a funding source (\(scheme))")
        }
    }
}
#endif
