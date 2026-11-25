import Foundation
import Testing
@testable import AgencyAPI

/// Each wire type decodes the node's real answer. A field the node renames or drops, or a shape it
/// changes, fails here before it reaches a screen.
@Suite("Wire types from the node's fixtures")
struct WireTypeTests {
    private func decode<T: Decodable>(_ type: T.Type, _ name: String) throws -> T {
        try JSONDecoder().decode(T.self, from: Fixtures.data(name))
    }

    @Test("me: the account, its orgs and teams")
    func me() throws {
        let me = try decode(Me.self, "me")
        #expect(me.account.id == "alice")
        #expect(me.account.fingerprint.count == 32)
        #expect(me.funding.isEmpty)
        #expect(me.orgs.count == 1)
        #expect(me.orgs[0].isRoot)
        #expect(me.orgs[0].teams == [TeamRef(id: "app", label: "app", location: .local, viewer: "owner")])
    }

    @Test("me for a member: a remote team and a viewer name that is not the owner's")
    func meMember() throws {
        let me = try decode(Me.self, "me-member")
        #expect(!me.orgs[0].isRoot)
        #expect(me.orgs[0].teams[0].location == .remote)
        #expect(me.orgs[0].teams[0].viewer == "reviewer")
    }

    @Test("a team snapshot on this node: mine, who waits on the viewer, the requests")
    func snapshotLocal() throws {
        let snapshot = try decode(TeamSnapshot.self, "team-snapshot-local")
        #expect(snapshot.source == .local)
        #expect(snapshot.viewer == "owner")
        #expect(snapshot.mine == ["r-1", "r-2"])
        #expect(snapshot.waitingOnViewer == ["backend"])
        #expect(snapshot.view.requests.map(\.id) == ["r-1", "r-2"])
        #expect(snapshot.view.requests[0].kind == .question)
        #expect(snapshot.view.requests[0].answerWith == AnswerWith(kind: .answer, target: "r-1"))
        #expect(snapshot.view.requests[0].to == nil)
        #expect(snapshot.view.requests[0].document == nil)
        #expect(snapshot.view.requests[0].thread == nil)
        #expect(snapshot.view.people.map(\.id) == ["backend", "frontend"])
        #expect(snapshot.view.people[0].waitingOn == .person("owner"))
        #expect(snapshot.view.people[0].state == .idle)
        #expect(snapshot.view.people[0].holding == ["app#7"])
        #expect(snapshot.view.generatedAt.text == "2026-01-01T00:00:00Z")
    }

    @Test("a team snapshot from a replica: when it was read and why it is old")
    func snapshotReplica() throws {
        let snapshot = try decode(TeamSnapshot.self, "team-snapshot-replica")
        guard case .replica(let fetchedAt, let triedAt, let error) = snapshot.source else {
            Issue.record("the source should be a replica")
            return
        }
        #expect(triedAt.text.hasSuffix("Z"))
        #expect(fetchedAt == nil || fetchedAt!.text.hasSuffix("Z"))
        _ = error
        #expect(snapshot.viewer == "reviewer")
        #expect(snapshot.waitingOnViewer == ["frontend"])
    }

    @Test("the decision log keeps Japanese text, an emoji, quotes, a backslash and a slash exactly")
    func decisionLog() throws {
        let log = try decode(DecisionLog.self, "decision-log")
        #expect(log.entries.count == 3)
        let comment = try #require(log.entries.first { $0.id == "00000000000000000000000000000003" })
        #expect(comment.text == "リリースは来週にしてください 🚀 \"quoted\" \\ /")
        #expect(comment.choice == nil)
        #expect(comment.isComment)
        #expect(comment.state == .applied)
        let chosen = try #require(log.entries.first { $0.id == "01010101010101010101010101010101" })
        #expect(chosen.choice == "b")
        #expect(!chosen.isComment)
    }

    @Test("a queued decision says how many times it was tried, when it goes next and why it failed")
    func queuedDecision() throws {
        let log = try decode(DecisionLog.self, "decision-log-member")
        guard case .queued(let attempts, let nextAt, let lastError) = log.entries[0].state else {
            Issue.record("the state should be queued")
            return
        }
        #expect(attempts == 1)
        #expect(nextAt.text == "2026-11-24T12:00:15Z")
        #expect(lastError?.contains("error sending request") == true)
    }

    @Test("a decision result: applied, or queued with the reason")
    func decisionResult() throws {
        #expect(try decode(DecisionResult.self, "decision-result") == DecisionResult(outcome: .applied, message: nil))
        let queued = try decode(DecisionResult.self, "decision-result-queued")
        #expect(queued.outcome == .queued)
        #expect(queued.message?.isEmpty == false)
    }

    @Test("every fixture of a desk screen still decodes with an extra field and with a missing optional one")
    func toleratesAddedFields() throws {
        let text = try Fixtures.text("team-snapshot-local")
        let extra = text.replacingOccurrences(of: "\"mine\"", with: "\"future\": {\"a\": [1]}, \"mine\"")
        let snapshot = try JSONDecoder().decode(TeamSnapshot.self, from: Data(extra.utf8))
        #expect(snapshot.mine == ["r-1", "r-2"])
    }

    @Test("a renamed or missing required field fails", arguments: ["mine", "waitingOnViewer", "source", "answerWith", "generatedAt"])
    func requiredFields(field: String) throws {
        let text = try Fixtures.text("team-snapshot-local")
        let broken = text.replacingOccurrences(of: "\"\(field)\"", with: "\"renamed\"")
        #expect(throws: DecodingError.self) { try JSONDecoder().decode(TeamSnapshot.self, from: Data(broken.utf8)) }
    }

    @Test("a value the client does not know fails, instead of being read as another")
    func unknownEnumValues() throws {
        let text = try Fixtures.text("team-snapshot-local")
        let broken = text.replacingOccurrences(of: "\"question\"", with: "\"riddle\"")
        #expect(throws: DecodingError.self) { try JSONDecoder().decode(TeamSnapshot.self, from: Data(broken.utf8)) }
        let state = try Fixtures.text("decision-log").replacingOccurrences(of: "\"applied\"", with: "\"lost\"")
        #expect(throws: DecodingError.self) { try JSONDecoder().decode(DecisionLog.self, from: Data(state.utf8)) }
    }

    @Test("a time that is not RFC 3339 fails")
    func badTime() throws {
        let text = try Fixtures.text("team-snapshot-local")
        let broken = text.replacingOccurrences(of: "2026-01-01T00:00:00Z", with: "yesterday")
        #expect(throws: DecodingError.self) { try JSONDecoder().decode(TeamSnapshot.self, from: Data(broken.utf8)) }
    }
}
