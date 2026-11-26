import AgencyAPI
import AgencyModel
import Foundation

/// Teams built from `testdata/teamview.sample.json`, the sample the Rust tests, the web client and
/// the harness share: a lead, a backend and a frontend agent, a docs agent that is away, and the
/// owner; three requests (a question, a proposal that names the owner, a decision the board waits on).
enum Teams {
    static let questionID = "r-20261124-110000-a1b2"
    static let proposalID = "r-20261124-113000-c3d4"
    static let decisionID = "app#14"

    private static func sample(from file: String = #filePath) throws -> [String: Any] {
        // Tests/AgencyModelTests/Teams.swift: five levels up is the repository root.
        var url = URL(fileURLWithPath: file)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        let data = try Data(contentsOf: url.appendingPathComponent("testdata/teamview.sample.json"))
        return try JSONSerialization.jsonObject(with: data) as! [String: Any]
    }

    /// A local team whose viewer goes by `viewer`. By default `mine` is what the node would list: the
    /// requests addressed to the viewer or to no one, and `waitingOnViewer` the people who wait on them.
    static func team(
        viewer: String? = "owner", mine: [String]? = nil, waitingOnViewer: [String]? = nil,
        source: [String: Any] = ["kind": "local"], addRequests: [[String: Any]] = []
    ) -> Team {
        Team(snapshot: snapshot(viewer: viewer, mine: mine, waitingOnViewer: waitingOnViewer, source: source, addRequests: addRequests))
    }

    /// The same, as the node's answer.
    static func snapshot(
        viewer: String? = "owner", mine: [String]? = nil, waitingOnViewer: [String]? = nil,
        source: [String: Any] = ["kind": "local"], addRequests: [[String: Any]] = [], addWork: [[String: Any]] = []
    ) -> TeamSnapshot {
        try! build(viewer: viewer, mine: mine, waitingOnViewer: waitingOnViewer, source: source, addRequests: addRequests, addWork: addWork)
    }

    private static func build(
        viewer: String?, mine: [String]?, waitingOnViewer: [String]?, source: [String: Any], addRequests: [[String: Any]], addWork: [[String: Any]]
    ) throws -> TeamSnapshot {
        var view = try sample()
        var requests = view["requests"] as! [[String: Any]]
        requests += addRequests
        view["requests"] = requests
        view["work"] = (view["work"] as? [[String: Any]] ?? []) + addWork
        let people = view["people"] as! [[String: Any]]
        let ownMine = requests.filter { ($0["to"] as? String ?? viewer) == viewer && viewer != nil }.map { $0["id"] as! String }
        let ownWaiting = people.filter { ($0["waitingOn"] as? [String: Any])?["person"] as? String == viewer && viewer != nil }.map { $0["id"] as! String }
        var snapshot: [String: Any] = [
            "view": view, "mine": mine ?? ownMine, "waitingOnViewer": waitingOnViewer ?? ownWaiting, "source": source,
        ]
        if let viewer { snapshot["viewer"] = viewer }
        let data = try JSONSerialization.data(withJSONObject: snapshot)
        return try JSONDecoder().decode(TeamSnapshot.self, from: data)
    }

    /// One more question, addressed to the person named `to`, with two options.
    static func request(
        id: String = "r-20261124-120000-e5f6", to: String?, title: String = "Should the nightly export include archived projects?",
        options: Bool = true, createdAt: String = "2026-11-24T12:00:00Z", blocks: [String] = [], optionText: String = "Include them"
    ) -> [String: Any] {
        var request: [String: Any] = [
            "id": id, "kind": "question", "title": title, "context": "", "from": "lead",
            "createdAt": createdAt, "blocks": blocks,
            "options": options ? [["key": "yes", "text": optionText, "isRecommended": false], ["key": "no", "text": "Leave them out", "isRecommended": false]] : [],
            "answerWith": ["kind": "answer", "target": id],
        ]
        if let to { request["to"] = to }
        return request
    }
}
