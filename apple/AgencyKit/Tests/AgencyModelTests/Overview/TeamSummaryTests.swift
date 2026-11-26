@testable import AgencyAPI
@testable import AgencyModel
import Testing

/// Ported from `web/src/overview/summary.test.ts`.
@Suite("Team summary and order")
struct TeamSummaryTests {
    private typealias T = OverviewTeams

    @Test("counts what the node lists as the viewer's, and calls the team “mine” however busy it is")
    func mineWins() {
        let team = T.team(people: [T.person("a", "working")], mine: ["r-1", "r-2"])
        let summary = TeamSummary(team: team)
        #expect(summary.attention == .mine)
        #expect(summary.mine == 2)
    }

    @Test("leaves the viewer out of the participants and counts who is working")
    func leavesViewerOut() {
        let team = T.team(people: [T.person("owner", "unknown"), T.person("a", "working"), T.person("b", "idle"), T.person("c", "working")])
        let summary = TeamSummary(team: team)
        #expect(summary.participants == 3)
        #expect(summary.working == 2)
        #expect(summary.attention == .active)
    }

    @Test("a team that does not address the viewer counts every participant")
    func noViewer() {
        let team = T.team(people: [T.person("owner", "idle", status: "x"), T.person("a", "working")], viewer: nil)
        #expect(TeamSummary(team: team).participants == 2)
    }

    @Test("is stuck when work is blocked, and says why in the reasons the team gives, each counted, in a fixed order")
    func stuck() {
        let team = T.team(
            people: [T.person("a", "working")],
            work: [
                T.blocked("x#1", "external"), T.blocked("x#2", "needsDecision"), T.blocked("x#3", "needsDecision"),
                T.blocked("x#4", "timeOrEquipment"), T.work("x#5"),
            ]
        )
        let summary = TeamSummary(team: team)
        #expect(summary.attention == .stuck)
        #expect(summary.blockedCount == 4)
        #expect(summary.blocked == [
            BlockedGroup(reason: .needsDecision, count: 2),
            BlockedGroup(reason: .external, count: 1),
            BlockedGroup(reason: .timeOrEquipment, count: 1),
        ])
    }

    @Test("is idle when nobody works and nothing is blocked, with what the quiet ones say, twice at most and once each")
    func idle() {
        let team = T.team(people: [
            T.person("a", "idle", status: "Nothing in Next"), T.person("b", "away", status: "Nothing in Next"),
            T.person("c", "idle", status: " Waiting for a build "), T.person("d", "idle", status: "Third line"),
            T.person("e", "idle", status: "   "),
        ])
        let summary = TeamSummary(team: team)
        #expect(summary.attention == .idle)
        #expect(summary.quiet == ["Nothing in Next", "Waiting for a build"])
    }

    @Test("a working participant's status is not a quiet line")
    func workingNotQuiet() {
        let team = T.team(people: [T.person("a", "working", status: "Building"), T.person("b", "idle", status: "Waiting")])
        #expect(TeamSummary(team: team).quiet == ["Waiting"])
    }

    @Test("is not idle for a team with no participants at all")
    func empty() {
        #expect(TeamSummary(team: T.team()).attention == .active)
    }

    // MARK: Order

    private var mineState: TeamState { .ready(T.team(people: [T.person("a", "working")], mine: ["r-1"])) }
    private var stuckState: TeamState { .ready(T.team(people: [T.person("a", "working")], work: [T.blocked("x#1", "external")])) }
    private var idleState: TeamState { .ready(T.team(people: [T.person("a", "idle", status: "Nothing in Next")])) }
    private var activeState: TeamState { .ready(T.team(people: [T.person("a", "working")])) }
    private let failed = TeamState.failed(reason: "no", signInEnded: false)

    @Test("puts the teams that wait on the viewer first, then stuck, a team that cannot be read, idle, the rest, and what still loads")
    func order() {
        let states: [(String, TeamState)] = [("loading", .loading), ("active", activeState), ("idle", idleState), ("failed", failed), ("stuck", stuckState), ("mine", mineState)]
        let entries = states.map { T.entry($0.0, $0.1) }
        #expect(OverviewOrder.teams(entries).map(\.ref.label) == ["mine", "stuck", "failed", "idle", "active", "loading"])
    }

    @Test("puts the team with more waiting on the viewer first, then by name")
    func weightThenName() {
        let two = TeamState.ready(T.team(mine: ["r-1", "r-2"]))
        let one = TeamState.ready(T.team(mine: ["r-3"]))
        let entries = [T.entry("b", one), T.entry("a", one), T.entry("z", two)]
        #expect(OverviewOrder.teams(entries).map(\.ref.label) == ["z", "a", "b"])
    }

    @Test("puts the team with more blocked work first within the stuck ones")
    func blockedWeight() {
        let few = TeamState.ready(T.team(work: [T.blocked("x#1", "external")]))
        let many = TeamState.ready(T.team(work: [T.blocked("x#1", "external"), T.blocked("x#2", "external")]))
        #expect(OverviewOrder.teams([T.entry("a", few), T.entry("b", many)]).map(\.ref.label) == ["b", "a"])
    }

    @Test("ranks an org by its most pressing team, and an org with no teams after every team that was read")
    func orgRank() {
        #expect(OverviewOrder.rank(ofOrgWith: [T.entry("a", activeState), T.entry("b", stuckState)]) == OverviewOrder.rank(stuckState))
        #expect(OverviewOrder.rank(ofOrgWith: []) > OverviewOrder.rank(activeState))
    }

    @Test("a team that cannot be read says whether the sign-in ended")
    func failedKeepsReason() {
        #expect(OverviewOrder.rank(.failed(reason: "x", signInEnded: true)) == OverviewOrder.rank(failed))
    }
}
