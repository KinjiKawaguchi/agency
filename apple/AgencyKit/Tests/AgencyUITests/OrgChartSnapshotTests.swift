#if canImport(SwiftUI) && os(macOS)
import AgencyAPI
import AgencyModel
import SwiftUI
import Testing
@testable import AgencyUI

/// The org chart drawn at phone width from a made-up org. The rule checked is the owner's: nothing is wider than
/// the screen. With `AGENCY_SNAPSHOT_DIR` set, each test leaves a PNG to look at.
@MainActor
@Suite("Org chart renderings", .serialized)
struct OrgChartSnapshotTests {
    private let width = Snapshot.phoneWidth

    private func expectFits(_ result: Snapshot.Result, _ name: String, sourceLocation: SourceLocation = #_sourceLocation) {
        #expect(!result.png.isEmpty, "\(name) was not drawn", sourceLocation: sourceLocation)
        #expect(result.size.width == width, "\(name) is \(result.size.width) points wide, the screen is \(width)", sourceLocation: sourceLocation)
        #expect(result.size.height > 40, sourceLocation: sourceLocation)
    }

    // MARK: Made-up orgs

    private func person(_ id: String, _ label: String, state: String = "working") -> [String: Any] {
        ["id": id, "label": label, "kind": "agent", "role": id == "lead" ? "lead" : "member", "state": state, "status": "", "holding": [String]()]
    }

    private func team(_ org: String, _ id: String, people: [[String: Any]], mine: [String] = [], blocked: Bool = false, source: [String: Any] = ["kind": "local"]) -> Team {
        let work: [[String: Any]] = blocked ? [["ref": "x#1", "title": "t", "status": "next", "blocked": ["reason": "external", "since": "2026-11-20"]]] : []
        let view: [String: Any] = ["org": org, "team": id, "label": id, "generatedAt": "2026-11-24T12:00:00Z", "people": people, "requests": [[String: Any]](), "work": work, "epics": [[String: Any]]()]
        let body: [String: Any] = ["view": view, "viewer": "owner", "mine": mine, "waitingOnViewer": [String](), "source": source]
        return Team(snapshot: try! JSONDecoder().decode(TeamSnapshot.self, from: JSONSerialization.data(withJSONObject: body)))
    }

    private func orgRef(_ id: String, _ label: String) -> OrgRef {
        try! JSONDecoder().decode(OrgRef.self, from: Data(#"{"id":"\#(id)","label":"\#(label)","isRoot":true,"teams":[]}"#.utf8))
    }

    private let definition = try! JSONDecoder().decode(OrgDefinition.self, from: Data(#"""
    {"org":"acme","label":"Acme",
     "departments":[
       {"id":"engineering","label":"Engineering","parent":null,"head":"alice"},
       {"id":"platform","label":"Platform","parent":"engineering","head":null},
       {"id":"storage","label":"Storage and the long department name that goes on and on","parent":"platform","head":null},
       {"id":"design","label":"Design","parent":null,"head":"carol"}],
     "people":[
       {"id":"alice","label":"Alice","account":"alice","department":"engineering","role":"member"},
       {"id":"bob","label":"Bob","account":"bob","department":"platform","role":"answerer"},
       {"id":"carol","label":"Carol with a very long name that does not fit on one line at all","account":"carol","department":"design","role":"member"}],
     "teams":[
       {"id":"app","label":"App","department":"engineering","node":"https://alice.example"},
       {"id":"build","label":"Build","department":"storage","node":"https://alice.example"},
       {"id":"site","label":"Site","department":"design","node":"https://alice.example"},
       {"id":"docs","label":"docs-team-with-an-unbroken-name-0123456789-abcdefghijklmnopqrstuvwxyz","department":"design","node":"https://alice.example"},
       {"id":"ghost","label":"Ghost","department":"design","node":"https://alice.example"}]}
    """#.utf8))

    private var teams: [Team] {
        let stale: [String: Any] = ["kind": "replica", "fetchedAt": "2026-11-24T10:00:00Z", "triedAt": "2026-11-24T12:00:00Z", "error": "refused"]
        return [
            team("acme", "app", people: [person("lead", "Lead"), person("backend", "Backend"), person("frontend", "Frontend", state: "idle"), person("docs", "Docs", state: "away"), person("a", "Alpha"), person("b", "Beta"), person("c", "Gamma"), person("d", "Delta")], mine: ["r-1", "r-2"]),
            team("acme", "build", people: [person("a", "Alpha")], blocked: true),
            team("acme", "site", people: [person("a", "Alpha", state: "idle")], source: stale),
            team("acme", "docs", people: []),
        ]
    }

    // MARK: Renderings

    @Test("the chart of every org, side by side under the account, fits the screen in light and dark")
    func allOrgs() {
        let orgs = OrgTreeBuilder.allOrgs(
            [
                OrgInput(org: orgRef("acme", "Acme"), definition: definition, failed: false),
                OrgInput(org: orgRef("lab", UIFixtures.unbrokenID), definition: nil, failed: false),
                OrgInput(org: orgRef("club", "Club"), definition: nil, failed: true),
            ],
            teams: teams, viewerAccount: "alice", loading: ["acme/ghost"]
        )
        let rows = OrgChartRow.allOrgs(account: "ALICE", orgs: orgs)
        let view = OrgChartContent(title: Ja.OrgChart.allTitle, orgs: [orgRef("acme", "Acme"), orgRef("lab", "Lab"), orgRef("club", "Club")], isAll: true, selected: nil, rows: rows)
        for scheme in [ColorScheme.light, .dark] {
            expectFits(Snapshot.capture("orgchart-all", view, scheme: scheme), "the chart of every org (\(scheme))")
        }
    }

    @Test("one org's chart, whole and focused on a department, fits the screen")
    func oneOrg() {
        let whole = OrgTreeBuilder.oneOrg(definition, teams: teams, viewerAccount: "alice", loading: ["acme/ghost"])
        let view = OrgChartContent(
            title: Ja.OrgChart.orgTitle("Acme"), orgs: [orgRef("acme", "Acme"), orgRef("lab", "Lab")], isAll: false, selected: "acme",
            rows: OrgChartRow.oneOrg(label: "Acme", orgID: "acme", tree: whole)
        )
        expectFits(Snapshot.capture("orgchart-one", view), "one org's chart")
        let focused = OrgTreeBuilder.oneOrg(definition, teams: teams, viewerAccount: "alice", focus: "platform")
        let focusedView = OrgChartContent(
            title: Ja.OrgChart.orgTitle("Acme"), orgs: [orgRef("acme", "Acme")], isAll: false, selected: "acme",
            rows: OrgChartRow.oneOrg(label: "Acme", orgID: "acme", tree: focused), isFocused: true
        )
        expectFits(Snapshot.capture("orgchart-focused", focusedView), "a focused department")
    }

    @Test("an empty department, an org that cannot be read and an account with no org fit the screen")
    func edges() {
        let empty = OrgTreeBuilder.oneOrg(
            try! JSONDecoder().decode(OrgDefinition.self, from: Data(#"{"org":"acme","label":"Acme","departments":[{"id":"e","label":"Empty","parent":null,"head":null}],"people":[],"teams":[]}"#.utf8)),
            teams: [], viewerAccount: "alice", focus: "e"
        )
        let view = VStack(alignment: .leading, spacing: 24) {
            OrgChartContent(title: "Acme", orgs: [orgRef("acme", "Acme")], isAll: false, selected: "acme", rows: OrgChartRow.oneOrg(label: "Acme", orgID: "acme", tree: empty), isFocused: true, isEmptyDepartment: true)
            OrgChartContent(title: "Acme", orgs: [orgRef("acme", "Acme")], isAll: false, selected: "acme", rows: [], isOrgUnavailable: true)
            OrgChartContent(title: "Acme", orgs: [orgRef("acme", "Acme")], isAll: false, selected: "acme", rows: [], isOrgLoading: true)
            OrgChartContent(title: Ja.OrgChart.title, orgs: [], isAll: false, selected: nil, rows: [])
        }
        expectFits(Snapshot.capture("orgchart-edges", view), "the edge cases")
    }
}
#endif
