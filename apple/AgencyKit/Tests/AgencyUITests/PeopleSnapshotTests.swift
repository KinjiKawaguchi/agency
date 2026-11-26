#if canImport(SwiftUI) && os(macOS)
import AgencyAPI
import AgencyModel
import SwiftUI
import Testing
@testable import AgencyUI

/// The participants and work screen drawn at phone width. The rule checked is the owner's: nothing is
/// wider than the screen. With `AGENCY_SNAPSHOT_DIR` set, each test leaves a PNG to look at.
@MainActor
@Suite("People renderings", .serialized)
struct PeopleSnapshotTests {
    private let now = UIFixtures.now
    private let width = Snapshot.phoneWidth

    private func expectFits(_ result: Snapshot.Result, _ name: String, sourceLocation: SourceLocation = #_sourceLocation) {
        #expect(!result.png.isEmpty, "\(name) was not drawn", sourceLocation: sourceLocation)
        #expect(result.size.width == width, "\(name) is \(result.size.width) points wide, the screen is \(width)", sourceLocation: sourceLocation)
        #expect(result.size.height > 40, sourceLocation: sourceLocation)
    }

    // MARK: Made-up teams

    private func person(_ id: String, _ label: String, kind: String = "agent", role: String = "member", state: String, status: String, holding: [String] = [], waitingOn: [String: Any]? = nil) -> [String: Any] {
        var person: [String: Any] = ["id": id, "label": label, "kind": kind, "role": role, "state": state, "status": status, "holding": holding, "since": "2026-11-24T11:40:00Z"]
        if let waitingOn { person["waitingOn"] = waitingOn }
        return person
    }

    private func work(_ ref: String, _ title: String, status: String, assignee: String? = nil, blocked: String? = nil) -> [String: Any] {
        var item: [String: Any] = ["ref": ref, "title": title, "status": status]
        if let assignee { item["assignee"] = assignee }
        if let blocked { item["blocked"] = ["reason": blocked, "since": "2026-11-23"] }
        if status == "inProgress" {
            item["eta"] = ["p50": "2026-11-25T03:00:00Z", "p80": "2026-11-25T09:00:00Z", "basis": "8 finished items", "isOverBudget": false]
        }
        return item
    }

    private func epic(_ ref: String, _ title: String, total: Int, done: Int, doing: Int, blocked: [[String: Any]] = [], at: String? = "2026-11-27T12:00:00Z") -> [String: Any] {
        var epic: [String: Any] = ["ref": ref, "title": title, "total": total, "done": done, "doing": doing, "remainingPoints": 3, "blocked": blocked, "basis": "No recent throughput to forecast from"]
        if let at { epic["forecast"] = ["days": 3, "at": at] }
        return epic
    }

    private var team: Team {
        let long = UIFixtures.unbrokenID
        let held = (1...14).map { "site-checkout#\($0)" }
        let people = [
            person("lead", "Lead", role: "lead", state: "working", status: "Reviewing app#12"),
            person("backend", "Backend", state: "working", status: "Permission prompt: run the database migration", holding: ["app#12"], waitingOn: ["kind": "person", "person": "owner"]),
            person("builder", UIFixtures.longJapaneseTitle, kind: "human", state: "working", status: UIFixtures.longJapaneseTitle, holding: held),
            person(long, long, state: "idle", status: long, waitingOn: ["kind": "lead"]),
            person("docs", "Docs", state: "away", status: "Session ended", holding: ["app#15"]),
            person("owner", "Alice", kind: "human", state: "unknown", status: ""),
        ]
        let works = (1...10).map { work("app#\($0 + 20)", "Task \($0): \(UIFixtures.longJapaneseTitle)", status: "inProgress", assignee: $0 % 2 == 0 ? "backend" : long) }
            + [work("app#14", "Remove the old export format", status: "next", blocked: "needsDecision"), work("app#15", "Document the session store", status: "backlog", assignee: "docs", blocked: "waitingOnPrevious")]
        let epics = [
            epic("app#10", "Persistent sessions", total: 4, done: 1, doing: 1, blocked: [["reason": "needsDecision", "count": 1], ["reason": "waitingOnPrevious", "count": 1]]),
            epic("app#20", UIFixtures.longJapaneseTitle, total: 0, done: 0, doing: 0, at: nil),
        ]
        let view: [String: Any] = [
            "org": "acme", "team": "app", "label": "App", "generatedAt": "2026-11-24T12:00:00Z", "people": people,
            "requests": [UIFixtures.request(id: "r-1", title: "Which database should the app use in production?", blocks: ["backend"])],
            "work": works, "epics": epics,
        ]
        let snapshot: [String: Any] = ["view": view, "viewer": "owner", "mine": ["r-1"], "waitingOnViewer": ["backend"], "source": ["kind": "local"]]
        return Team(snapshot: try! JSONDecoder().decode(TeamSnapshot.self, from: JSONSerialization.data(withJSONObject: snapshot)))
    }

    // MARK: Renderings

    @Test("a team with every kind of participant, a long list of work and epics fits the screen in light and dark")
    func everything() {
        let view = PeopleContent(sections: [PeopleSection(team: team)], failures: ["acme/docs": "connection refused"], now: now)
        for scheme in [ColorScheme.light, .dark] {
            expectFits(Snapshot.capture("people", view, scheme: scheme), "the people screen (\(scheme))")
        }
    }

    @Test("the shared sample team fits the screen")
    func sample() {
        let view = PeopleContent(sections: [PeopleSection(team: UIFixtures.team())], failures: [:], now: now)
        expectFits(Snapshot.capture("people-sample", view), "the sample team")
    }

    @Test("no teams says so")
    func empty() {
        expectFits(Snapshot.capture("people-empty", PeopleContent(sections: [], failures: [:], now: now)), "no teams")
    }
}
#endif
