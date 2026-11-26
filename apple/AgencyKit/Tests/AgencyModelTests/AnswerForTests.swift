import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

@Suite("Answering for a seat or an external party")
struct AnswerForTests {
    private let seat = Teams.request(id: "r-seat", to: "finance")

    @Test("the node's map of whom a request is for is read, and a node that says none gives none")
    func wire() {
        let named = Teams.snapshot(mine: ["r-seat"], addRequests: [seat], relayedFor: ["r-seat": "Dan (tax)"])
        #expect(named.relayedFor == ["r-seat": "Dan (tax)"])
        #expect(Teams.snapshot().relayedFor == nil)
        #expect(Team(snapshot: Teams.snapshot()).relayedFor.isEmpty)
    }

    @Test("a request of the viewer's addressed to a seat says whom it is for; nobody else's request, and a request with no entry, say nothing")
    func deskItem() throws {
        let team = Teams.team(mine: ["r-seat", Teams.questionID], addRequests: [seat], relayedFor: ["r-seat": "Dan (tax)", "r-others": "Not mine"])
        let items = Desk.openItems([team])
        #expect(try #require(items.first { $0.request.id == "r-seat" }).answersFor == "Dan (tax)")
        #expect(try #require(items.first { $0.request.id == Teams.questionID }).answersFor == nil, "the owner's own request")
        let notMine = Teams.team(mine: [Teams.questionID], addRequests: [seat], relayedFor: ["r-seat": "Dan (tax)"])
        #expect(try #require(Desk.openItems([notMine]).first { $0.request.id == "r-seat" }).answersFor == nil, "only the viewer's own requests say it")
    }

    @Test("an empty label makes no line, and a long or odd one is kept as it came")
    func labels() throws {
        let empty = Teams.team(mine: ["r-seat"], addRequests: [seat], relayedFor: ["r-seat": ""])
        #expect(try #require(Desk.openItems([empty]).first { $0.request.id == "r-seat" }).answersFor == nil)
        let spaces = Teams.team(mine: ["r-seat"], addRequests: [seat], relayedFor: ["r-seat": "  \n "])
        #expect(try #require(Desk.openItems([spaces]).first { $0.request.id == "r-seat" }).answersFor == nil, "only spaces is empty too")
        let odd = "<b>Dan</b> & " + String(repeating: "税務", count: 40)
        let team = Teams.team(mine: ["r-seat"], addRequests: [seat], relayedFor: ["r-seat": odd])
        #expect(try #require(Desk.openItems([team]).first { $0.request.id == "r-seat" }).answersFor == odd)
    }

    @Test("the words are the web's")
    func words() {
        #expect(Ja.Desk.answerFor("Dan (tax)") == "Dan (tax) の代理で答える")
        #expect(Ja.Recent.answeredFor("Dan (tax)") == "Dan (tax) の代理で答えました。")
    }

    private func entry(_ relayed: String) throws -> DecisionEntry {
        let json = """
        {"entries":[{"choice":"a","decidedAt":"2026-11-24T12:00:00Z","id":"01","kind":"answer","org":"acme","state":{"kind":"applied"},
        "target":"r-1","team":"app","text":""\(relayed)}]}
        """
        return try JSONDecoder().decode(DecisionLog.self, from: Data(json.utf8)).entries[0]
    }

    @Test("a decision log entry says whom the decision answered for, when the node does")
    func logEntry() throws {
        #expect(try entry(#","relayedFor":{"id":"dan","label":"Dan (tax)"}"#).answeredFor == "Dan (tax)")
        #expect(try entry(#","relayedFor":{"id":"dan","label":"Dan (tax)"}"#).relayedFor == RelayedFor(id: "dan", label: "Dan (tax)"))
        #expect(try entry("").answeredFor == nil, "the owner's own, or a team on another node")
        #expect(try entry(#","relayedFor":{"id":"dan","label":""}"#).answeredFor == nil, "an empty label makes no line")
        #expect(try entry(#","relayedFor":{"id":"dan","label":"   "}"#).answeredFor == nil, "a label of only spaces makes none either")
        #expect(try entry(#","relayedFor":{"id":"dan","label":" Dan (tax) "}"#).answeredFor == "Dan (tax)")
    }
}
