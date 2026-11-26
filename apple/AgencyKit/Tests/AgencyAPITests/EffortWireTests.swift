import Foundation
import Testing
@testable import AgencyAPI

/// The shared sample (`testdata/effort.sample.json`) is the contract the node's route and the clients share.
@Suite("Effort wire types")
struct EffortWireTests {
    private func sample(from file: String = #filePath) throws -> Data {
        var url = URL(fileURLWithPath: file)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        return try Data(contentsOf: url.appendingPathComponent("testdata/effort.sample.json"))
    }

    private func decode(_ json: String) throws -> Effort {
        try JSONDecoder().decode(Effort.self, from: Data(json.utf8))
    }

    private func answer(team: String) -> String {
        #"{"org":"acme","days":14,"from":"2026-11-11","until":"2026-11-24","generatedAt":"2026-11-24T12:00:00Z","teams":[\#(team)],"unavailable":[]}"#
    }

    private let period = #""from":"2026-11-11","until":"2026-11-24""#

    @Test("the shared sample decodes: the cost and tokens of each type, and the cost of each type for each day")
    func shared() throws {
        let effort = try JSONDecoder().decode(Effort.self, from: try sample())
        #expect(effort.org == "acme")
        #expect(effort.days == 14)
        #expect(effort.teams.map(\.team) == ["app"])
        let app = effort.teams[0]
        #expect(app.from == "2026-11-11")
        #expect(app.until == "2026-11-24")
        #expect(app.byType[.newFeature]?.costUsd == 8.1)
        #expect(app.byType[.newFeature]?.tokens == EffortTokens(input: 90_000, output: 30_000, cacheRead: 1_800_000, cacheWrite: 120_000))
        #expect(app.byType[.unclassified]?.costUsd == 0)
        #expect(app.byType.count == 6)
        #expect(app.byDay.map(\.date) == ["2026-11-23", "2026-11-24"])
        #expect(app.byDay[0].costUsd[.maintenance] == 2.0)
        #expect(effort.unavailable.isEmpty)
    }

    @Test("a work type the node leaves out counts as 0, in the totals and in a day")
    func missing() throws {
        let effort = try decode(answer(team: #"{"team":"app",\#(period),"byType":{"newFeature":{"costUsd":5,"tokens":{"input":1,"output":2,"cacheRead":3,"cacheWrite":4}}},"byDay":[{"date":"2026-11-24","costUsd":{"newFeature":5}}]}"#))
        let team = effort.teams[0]
        #expect(team.byType[.incident] == TypeUsage(costUsd: 0, tokens: EffortTokens(input: 0, output: 0, cacheRead: 0, cacheWrite: 0)))
        #expect(team.byType[.newFeature]?.costUsd == 5)
        #expect(team.byDay[0].costUsd == [.newFeature: 5, .improvement: 0, .maintenance: 0, .debt: 0, .incident: 0, .unclassified: 0])
    }

    @Test("a team with no types and no days at all is a team with nothing spent")
    func nothing() throws {
        let effort = try decode(answer(team: #"{"team":"app",\#(period)}"#))
        #expect(effort.teams[0].byDay.isEmpty)
        #expect(EffortType.allCases.allSatisfy { effort.teams[0].byType[$0]?.costUsd == 0 })
    }

    @Test("a cost that is negative or not a number refuses the whole answer, in a type or in a day")
    func refused() {
        #expect(throws: DecodingError.self) { try decode(answer(team: #"{"team":"a",\#(period),"byType":{"newFeature":{"costUsd":-1}},"byDay":[]}"#)) }
        #expect(throws: DecodingError.self) { try decode(answer(team: #"{"team":"a",\#(period),"byType":{"newFeature":{"costUsd":"5"}},"byDay":[]}"#)) }
        #expect(throws: DecodingError.self) { try decode(answer(team: #"{"team":"a",\#(period),"byType":{},"byDay":[{"date":"2026-11-24","costUsd":{"debt":-3}}]}"#)) }
        #expect(throws: DecodingError.self) { try decode(answer(team: #"{"team":"a",\#(period),"byType":{"debt":{"costUsd":1,"tokens":{"input":-1}}},"byDay":[]}"#)) }
    }

    @Test("a team has its own period, and the answer's list of teams that could not answer is always there")
    func required() {
        #expect(throws: DecodingError.self) { try decode(answer(team: #"{"team":"a","byType":{},"byDay":[]}"#)) }
        #expect(throws: DecodingError.self) { try decode(#"{"org":"acme","days":14,"from":"2026-11-11","until":"2026-11-24","generatedAt":"2026-11-24T12:00:00Z","teams":[]}"#) }
    }
}

@Suite("Org plans")
struct OrgPlansTests {
    private let allocation = #"{"newFeature":50,"improvement":20,"maintenance":20,"debt":10,"incident":0}"#

    private func plans(_ teams: String) throws -> OrgPlans {
        try JSONDecoder().decode(OrgPlans.self, from: Data(#"{"org":"acme","label":"Acme","version":4,"teams":[\#(teams)]}"#.utf8))
    }

    @Test("a plan with the shape the protocol allows is read, for the team that has it")
    func valid() throws {
        let read = try plans(#"{"id":"other"},{"id":"app","plan":{"allocation":\#(allocation),"weeklyBudgetUsd":140}}"#)
        #expect(read.byTeam == ["app": TeamPlan(allocation: [.newFeature: 50, .improvement: 20, .maintenance: 20, .debt: 10, .incident: 0], weeklyBudgetUsd: 140)])
    }

    @Test("a team that has none, a definition without teams, and the node's recorded definition give no plan")
    func none() throws {
        #expect(try plans(#"{"id":"app"}"#).byTeam.isEmpty)
        #expect(try JSONDecoder().decode(OrgPlans.self, from: Data(#"{"org":"acme"}"#.utf8)).byTeam.isEmpty)
        #expect(try JSONDecoder().decode(OrgPlans.self, from: Data(#"{"teams":"x"}"#.utf8)).byTeam.isEmpty)
        let data = try Fixtures.data("org")
        #expect(try JSONDecoder().decode(OrgPlans.self, from: data).byTeam.isEmpty)
    }

    @Test("shares that do not sum to 100 give no plan")
    func notHundred() throws {
        #expect(try plans(#"{"id":"app","plan":{"allocation":{"newFeature":50,"improvement":20,"maintenance":20,"debt":11,"incident":0},"weeklyBudgetUsd":140}}"#).byTeam.isEmpty)
        #expect(try plans(#"{"id":"app","plan":{"allocation":{"newFeature":50,"improvement":20,"maintenance":20,"debt":9,"incident":0},"weeklyBudgetUsd":140}}"#).byTeam.isEmpty)
    }

    @Test("shares and budgets that are not whole, are negative, or are missing give no plan")
    func malformed() throws {
        let cases = [
            #"{"allocation":{"newFeature":49.5,"improvement":20,"maintenance":20,"debt":10.5,"incident":0},"weeklyBudgetUsd":140}"#,
            #"{"allocation":{"newFeature":110,"improvement":20,"maintenance":20,"debt":-10,"incident":0},"weeklyBudgetUsd":140}"#,
            #"{"allocation":\#(allocation),"weeklyBudgetUsd":12.5}"#,
            #"{"allocation":\#(allocation),"weeklyBudgetUsd":-1}"#,
            #"{"allocation":\#(allocation)}"#,
            #"{"allocation":{"newFeature":100},"weeklyBudgetUsd":1}"#,
            #"{"allocation":"all","weeklyBudgetUsd":1}"#,
        ]
        for plan in cases {
            #expect(try plans(#"{"id":"app","plan":\#(plan)}"#).byTeam.isEmpty, "\(plan)")
        }
    }

    @Test("one team's bad plan does not blank another team's good one")
    func independent() throws {
        let read = try plans(#"{"id":"bad","plan":{"allocation":{"newFeature":1},"weeklyBudgetUsd":1}},{"id":"good","plan":{"allocation":\#(allocation),"weeklyBudgetUsd":0}}"#)
        #expect(Set(read.byTeam.keys) == ["good"])
        #expect(read.byTeam["good"]?.weeklyBudgetUsd == 0)
    }
}
