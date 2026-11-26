@testable import AgencyAPI
@testable import AgencyModel
import Foundation

/// Teams for the overview's tests, built from the node's answer with the people and work this test needs.
enum OverviewTeams {
    static func person(_ id: String, _ state: String, status: String = "") -> [String: Any] {
        ["id": id, "label": id, "kind": "agent", "role": "member", "state": state, "status": status, "holding": [String]()]
    }

    static func blocked(_ ref: String, _ reason: String) -> [String: Any] {
        ["ref": ref, "title": ref, "status": "next", "blocked": ["reason": reason, "since": "2026-11-20"]]
    }

    static func work(_ ref: String) -> [String: Any] {
        ["ref": ref, "title": ref, "status": "next"]
    }

    static func snapshot(
        org: String = "acme", team: String = "app", people: [[String: Any]] = [], work: [[String: Any]] = [],
        viewer: String? = "owner", mine: [String] = [], source: [String: Any] = ["kind": "local"]
    ) -> TeamSnapshot {
        let view: [String: Any] = [
            "org": org, "team": team, "label": team, "generatedAt": "2026-11-24T12:00:00Z",
            "people": people, "requests": [[String: Any]](), "work": work, "epics": [[String: Any]](),
        ]
        var body: [String: Any] = ["view": view, "mine": mine, "waitingOnViewer": [String](), "source": source]
        if let viewer { body["viewer"] = viewer }
        return try! JSONDecoder().decode(TeamSnapshot.self, from: JSONSerialization.data(withJSONObject: body))
    }

    static func team(
        people: [[String: Any]] = [], work: [[String: Any]] = [], viewer: String? = "owner", mine: [String] = [],
        source: [String: Any] = ["kind": "local"]
    ) -> Team {
        Team(snapshot: snapshot(people: people, work: work, viewer: viewer, mine: mine, source: source))
    }

    static func ref(_ id: String, label: String? = nil) -> TeamRef {
        try! JSONDecoder().decode(TeamRef.self, from: Data(#"{"id":"\#(id)","label":"\#(label ?? id)","location":"local","viewer":"owner"}"#.utf8))
    }

    static func entry(_ label: String, _ state: TeamState, org: String = "acme") -> TeamEntry {
        TeamEntry(org: org, ref: ref(label, label: label), state: state)
    }
}
