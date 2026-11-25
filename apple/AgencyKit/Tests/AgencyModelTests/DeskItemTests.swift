import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

/// Ported from the web client's `desk/items.test.ts`: the phone makes the same rules.
@Suite("Desk items")
struct DeskItemTests {
    private let team = Teams.team()

    @Test("orders by group, then by how many people are stopped, then oldest first")
    func order() {
        let items = Desk.openItems([team])
        #expect(items.map { [String(describing: $0.group), $0.request.title] } == [
            ["proposal", "Add a second frontend agent"],
            ["question", "Which database should the app use in production?"],
            ["pendingDecision", "Drop support for the old export format?"],
        ])
    }

    @Test("a request with no addressee is the owner's")
    func absentAddressee() {
        #expect(Desk.openItems([team]).allSatisfy { $0.isMine })
    }

    @Test("a request addressed to someone else is in its own group, last, with the person it is addressed to")
    func others() {
        let items = Desk.openItems([Teams.team(addRequests: [Teams.request(to: "frontend")])])
        #expect(items.map(\.group) == [.proposal, .question, .pendingDecision, .others])
        #expect(items.last?.decider?.label == "Frontend")
        #expect(items.last?.isMine == false)
    }

    @Test("the viewer is whoever the node names, so a request with no addressee is theirs")
    func viewerInATeam() {
        let items = Desk.openItems([Teams.team(viewer: "lead")])
        #expect(items.map { [$0.request.title, String($0.isMine)] } == [
            ["Which database should the app use in production?", "true"],
            ["Drop support for the old export format?", "true"],
            ["Add a second frontend agent", "false"],
        ])
    }

    @Test("whose a request is, is the node's call: what it lists as the viewer's is theirs, whatever `to` says")
    func nodesCall() {
        let extra = Teams.request(to: "someone-unmapped")
        let ids = [Teams.questionID, Teams.proposalID, Teams.decisionID, extra["id"] as! String]
        let items = Desk.openItems([Teams.team(mine: ids, addRequests: [extra])])
        #expect(items.allSatisfy { $0.isMine })
        #expect(items.first { $0.request.id == extra["id"] as? String }?.group == .question)
    }

    @Test("a request is not the viewer's because `to` is absent, when the node does not list it")
    func notOnAGuess() {
        let items = Desk.openItems([Teams.team(viewer: "lead", mine: [])])
        #expect(items.map(\.isMine) == [false, false, false])
        #expect(items.map(\.group) == [.others, .others, .others])
    }

    @Test("the pile is every team's requests together, and the count is the viewer's")
    func severalTeams() {
        let other = Teams.team(mine: [], addRequests: [])
        #expect(Desk.openItems([team, other]).count == 6)
        #expect(Desk.waitingCount([team, other]) == 3)
        #expect(Desk.waitingCount([team]) == 3)
        #expect(Desk.waitingCount([]) == 0)
    }

    @Test("a request listed as mine but missing from the view is not counted")
    func countsOnlyWhatIsThere() {
        #expect(Desk.waitingCount([Teams.team(mine: ["nope", Teams.questionID])]) == 1)
    }

    @Test("within a group the request that stops more people comes first, even if it is newer")
    func morePeopleFirst() {
        let quiet = Teams.request(id: "r-quiet", to: nil, createdAt: "2026-11-24T09:00:00Z")
        let items = Desk.openItems([Teams.team(addRequests: [quiet])])
        let questions = items.filter { $0.group == .question }.map(\.request.id)
        #expect(questions == [Teams.questionID, "r-quiet"])
    }

    @Test("with the same people stopped, the oldest comes first")
    func oldestFirst() {
        let newer = Teams.request(id: "r-new", to: nil, createdAt: "2026-11-24T13:00:00Z")
        let older = Teams.request(id: "r-old", to: nil, createdAt: "2026-11-24T10:00:00Z")
        let items = Desk.openItems([Teams.team(addRequests: [newer, older])])
        let questions = items.filter { $0.group == .question && $0.blockedPeople.isEmpty }.map(\.request.id)
        #expect(questions == ["r-old", "r-new"])
    }

    @Test("the person a request is addressed to is the viewer when it is theirs")
    func deciderIsTheViewer() throws {
        let owner = try #require(Desk.openItems([team]).first { $0.request.id == Teams.questionID })
        #expect(owner.decider?.id == "owner")
        let lead = try #require(Desk.openItems([Teams.team(viewer: "lead")]).first { $0.request.id == Teams.questionID })
        #expect(lead.decider?.id == "lead")
    }

    @Test("ties keep the order the node gave")
    func stable() {
        let first = Teams.request(id: "r-a", to: nil)
        let second = Teams.request(id: "r-b", to: nil)
        let items = Desk.openItems([Teams.team(addRequests: [first, second])])
        let ids = items.map(\.request.id)
        #expect(ids.firstIndex(of: "r-a")! < ids.firstIndex(of: "r-b")!)
    }

    @Test("the item's id is the one a notification's deep link carries")
    func itemID() throws {
        let item = try #require(Desk.openItems([team]).first { $0.request.id == Teams.questionID })
        #expect(item.id == "acme:app:\(Teams.questionID)")
        #expect(item.id == DeskItem.key(org: "acme", team: "app", request: Teams.questionID))
    }

    @Test("blocked people are the people who wait, not the work items")
    func blocked() throws {
        let question = try #require(Desk.openItems([team]).first { $0.group == .question })
        #expect(question.request.blocks == ["backend", "app#12"])
        #expect(question.blockedPeople.map(\.label) == ["Backend"])
    }
}

@Suite("Situation and presence")
struct SituationTests {
    @Test("counts the other people, not the owner")
    func counts() {
        let team = Teams.team()
        let situation = Situation(teams: [team], items: Desk.openItems([team]))
        #expect(situation == Situation(waitingOnYou: 1, working: 2, total: 4, mine: 3))
    }

    @Test("a team that does not address the viewer has nobody waiting on them, and nothing is theirs")
    func notAddressed() {
        let team = Teams.team(viewer: nil)
        let situation = Situation(teams: [team], items: Desk.openItems([team]))
        #expect(situation.waitingOnYou == 0)
        #expect(situation.mine == 0)
    }

    @Test("it counts the people the node lists as waiting on the viewer")
    func nodesList() {
        let team = Teams.team(waitingOnViewer: ["backend", "lead"])
        #expect(Situation(teams: [team], items: []).waitingOnYou == 2)
    }

    @Test("presence: needs you beats everything, then away, then waiting, then working, then idle")
    func presence() throws {
        let team = Teams.team()
        func presence(_ id: String, waiting: Set<String> = []) throws -> Presence {
            Presence(of: try #require(team.person(named: id)), waitingOnViewer: waiting)
        }
        #expect(try presence("backend", waiting: ["backend"]) == .needsYou)
        #expect(try presence("docs") == .away)
        #expect(try presence("frontend") == .waiting)
        #expect(try presence("lead") == .working)
        #expect(try presence("owner") == .idle)
        #expect(try presence("docs", waiting: ["docs"]) == .needsYou)
    }
}
