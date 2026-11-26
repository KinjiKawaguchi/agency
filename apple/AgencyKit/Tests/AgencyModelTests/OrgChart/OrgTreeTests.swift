@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

/// An org made up the way the web's tests make it: departments nested by parent, members and teams placed in them.
enum OrgFixtures {
    static func definition(_ json: String) -> OrgDefinition { try! JSONDecoder().decode(OrgDefinition.self, from: Data(json.utf8)) }

    static let acme = definition(#"""
    {"org":"acme","label":"Acme",
     "departments":[
       {"id":"engineering","label":"Engineering","parent":null,"head":"seat-alice"},
       {"id":"platform","label":"Platform","parent":"engineering","head":null},
       {"id":"design","label":"Design","parent":null,"head":"carol"}],
     "people":[
       {"id":"seat-alice","label":"Alice","account":"alice","department":"engineering","role":"member"},
       {"id":"bob","label":"Bob","account":"bob","department":"platform","role":"answerer"},
       {"id":"carol","label":"Carol","account":"carol","department":"design","role":"member"}],
     "teams":[
       {"id":"app","label":"App","department":"engineering","node":"https://alice.example"},
       {"id":"site","label":"Site","department":"design","node":"https://alice.example"}]}
    """#)

    static func orgRef(_ id: String, _ label: String) -> OrgRef {
        try! JSONDecoder().decode(OrgRef.self, from: Data(#"{"id":"\#(id)","label":"\#(label)","isRoot":true,"teams":[]}"#.utf8))
    }

    /// The sample team of `acme`/`app`: it has requests waiting on the viewer, so it is the team that needs them.
    static var app: Team { Teams.team() }
}

@Suite("Org tree")
struct OrgTreeTests {
    private let teams = [OrgFixtures.app]
    private func tree(_ focus: String? = nil, teams: [Team]? = nil) -> OneOrgTree {
        OrgTreeBuilder.oneOrg(OrgFixtures.acme, teams: teams ?? self.teams, viewerAccount: "alice", focus: focus)
    }

    private func department(_ children: [UnitChild], _ id: String) -> DepartmentNode? {
        for case .department(let node) in children where node.id == id { return node }
        return nil
    }

    private func kinds(_ children: [UnitChild]) -> [String] {
        children.map { child in
            switch child {
            case .department: "department"
            case .team: "team"
            case .person: "person"
            }
        }
    }

    @Test("nests departments, teams and members under their department, in that order")
    func nesting() {
        let tree = tree()
        #expect(kinds(tree.children) == ["department", "department"])
        let engineering = department(tree.children, "engineering")
        #expect(kinds(engineering?.children ?? []) == ["department", "team", "person"])
        #expect(kinds(department(engineering?.children ?? [], "platform")?.children ?? []) == ["person"])
    }

    @Test("names the head of a department")
    func heads() {
        let tree = tree()
        #expect(department(tree.children, "engineering")?.head?.label == "Alice")
        #expect(department(tree.children, "design")?.head?.label == "Carol")
        #expect(department(department(tree.children, "engineering")?.children ?? [], "platform")?.head == nil)
    }

    @Test("marks the viewer by account, not by name")
    func viewer() {
        let engineering = department(tree().children, "engineering")
        let people = (engineering?.children ?? []).compactMap { child -> Bool? in
            if case .person(let node) = child { node.isYou } else { nil }
        }
        #expect(people == [true])
    }

    @Test("lists the agents of a team, counts the requests that are the viewer's, rolled up the tree")
    func signals() {
        let engineering = department(tree().children, "engineering")
        guard case .team(let app)? = engineering?.children.first(where: { if case .team = $0 { true } else { false } }) else {
            Issue.record("no team under engineering")
            return
        }
        #expect(app.agents.map(\.id) == ["lead", "backend", "frontend", "docs"])
        #expect(app.agents.first { $0.isLead }?.person.id == "lead")
        #expect(app.working == 2)
        #expect(app.signals?.mine == OrgFixtures.app.mine.count)
        #expect(OrgFixtures.app.mine.count > 0)
        #expect(engineering?.mine == OrgFixtures.app.mine.count)
        #expect(department(tree().children, "design")?.mine == 0)
    }

    @Test("keeps a team whose view could not be read, without agents")
    func unreadTeam() {
        let design = department(tree().children, "design")
        guard case .team(let site)? = design?.children.first(where: { if case .team = $0 { true } else { false } }) else {
            Issue.record("no team under design")
            return
        }
        #expect(site.team == nil)
        #expect(site.agents.isEmpty)
        #expect(site.signals == nil)
        #expect(!site.isLoading)
    }

    @Test("a team still being read is loading, not unavailable")
    func loading() {
        let tree = OrgTreeBuilder.oneOrg(OrgFixtures.acme, teams: [], viewerAccount: "alice", loading: ["acme/site"])
        let design = department(tree.children, "design")
        guard case .team(let site)? = design?.children.first(where: { if case .team = $0 { true } else { false } }) else { return }
        #expect(site.isLoading)
    }

    @Test("focuses on one department and what is below it, and shows the whole org when the focus is unknown")
    func focus() {
        let focused = tree("engineering")
        #expect(focused.focus?.id == "engineering")
        #expect(kinds(focused.children) == ["department", "team", "person"])
        #expect(tree("nowhere").focus == nil)
        #expect(department(tree("platform").children, "platform") == nil)
        #expect(tree("platform").focus?.id == "platform")
    }

    @Test("leaves out departments that cannot be reached from the top, such as a loop of parents")
    func loop() {
        let looping = OrgFixtures.definition(#"{"org":"acme","label":"Acme","departments":[{"id":"a","label":"A","parent":"b","head":null},{"id":"b","label":"B","parent":"a","head":null}],"people":[],"teams":[]}"#)
        #expect(OrgTreeBuilder.oneOrg(looping, teams: [], viewerAccount: "alice").children.isEmpty)
    }

    @Test("a team is matched to its own org: the same team id in two orgs does not borrow a view")
    func ownOrg() {
        let lab = OrgFixtures.definition(#"{"org":"lab","label":"Lab","departments":[{"id":"main","label":"Main","parent":null,"head":null}],"people":[],"teams":[{"id":"app","label":"App","department":"main","node":"https://alice.example"}]}"#)
        let orgs = OrgTreeBuilder.allOrgs(
            [OrgInput(org: OrgFixtures.orgRef("acme", "Acme"), definition: OrgFixtures.acme, failed: false), OrgInput(org: OrgFixtures.orgRef("lab", "Lab"), definition: lab, failed: false)],
            teams: teams, viewerAccount: "alice"
        )
        #expect(orgs.map { [$0.id, "\($0.state)"] } == [["acme", "ready"], ["lab", "ready"]])
        #expect(orgs[0].mine == OrgFixtures.app.mine.count)
        #expect(orgs[1].mine == 0)
        guard case .department(let main)? = orgs[1].children.first, case .team(let app)? = main.children.first else { Issue.record("no team in lab"); return }
        #expect(app.signals == nil)
    }

    @Test("counts nothing for a team the viewer has no view of, and keeps an org that is loading or could not be read")
    func states() {
        let acme = OrgFixtures.orgRef("acme", "Acme"), lab = OrgFixtures.orgRef("lab", "Lab"), club = OrgFixtures.orgRef("club", "Club")
        let orgs = OrgTreeBuilder.allOrgs(
            [OrgInput(org: acme, definition: OrgFixtures.acme, failed: false), OrgInput(org: lab, definition: nil, failed: false), OrgInput(org: club, definition: nil, failed: true)],
            teams: [], viewerAccount: "alice"
        )
        #expect(orgs.map(\.state) == [.ready, .loading, .failed])
        #expect(orgs[0].mine == 0)
        #expect(orgs[1].children.isEmpty)
        #expect(orgs[2].children.isEmpty)
    }
}
