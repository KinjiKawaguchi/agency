import Foundation
import Testing
@testable import AgencyAPI

@Suite("The weekly budget answer")
struct BudgetWireTests {
    private func decode(_ json: String) throws -> BudgetStatus {
        try JSONDecoder().decode(BudgetStatus.self, from: Data(json.utf8))
    }

    @Test("the node's own sample is read as it is: the week, each part's cap, spend and state, and the org read only in part")
    func sample() throws {
        let status = try JSONDecoder().decode(BudgetStatus.self, from: Fixtures.sample("budget"))
        #expect(status.weekStart == "2026-11-23")
        #expect(status.team == BudgetPart(capUsd: 200, spentUsd: 163.2, state: .warning))
        #expect(status.org == BudgetPart(capUsd: 500, spentUsd: 311, state: .ok, isPartial: true))
    }

    @Test("`partial` is left out when false and only ever true; a cap is left out for a team with no plan")
    func optional() throws {
        let status = try decode(#"{"weekStart":"2026-11-23","team":{"spentUsd":5,"state":"ok"},"org":{"spentUsd":0,"state":"ok","partial":true}}"#)
        #expect(status.team == BudgetPart(capUsd: nil, spentUsd: 5, state: .ok))
        #expect(!status.team.isPartial && status.org.isPartial)
        #expect(status.org.capUsd == nil)
        #expect(try decode(#"{"weekStart":"w","team":{"spentUsd":1,"state":"ok","partial":false},"org":{"spentUsd":1,"state":"ok"}}"#).team.isPartial == false)
    }

    @Test("a state the app does not know is read as unknown, and never as a state it knows")
    func unknownState() throws {
        let status = try decode(#"{"weekStart":"2026-11-23","team":{"capUsd":1,"spentUsd":1,"state":"critical"},"org":{"spentUsd":1,"state":"ok"}}"#)
        #expect(status.team.state == .unknown)
    }

    @Test("a spend that is missing is not read as nothing spent: the answer is refused", arguments: [
        #"{"weekStart":"w","team":{"capUsd":1,"state":"ok"},"org":{"spentUsd":1,"state":"ok"}}"#,
        #"{"weekStart":"w","team":{"capUsd":1,"spentUsd":1,"state":"ok"},"org":{"state":"ok"}}"#,
        #"{"weekStart":"w","team":{"capUsd":1,"spentUsd":"1","state":"ok"},"org":{"spentUsd":1,"state":"ok"}}"#,
        #"{"weekStart":"w","team":{"capUsd":1,"spentUsd":1},"org":{"spentUsd":1,"state":"ok"}}"#,
        #"{"team":{"capUsd":1,"spentUsd":1,"state":"ok"},"org":{"spentUsd":1,"state":"ok"}}"#,
        #"{"weekStart":"w","org":{"spentUsd":1,"state":"ok"}}"#,
    ])
    func refused(json: String) {
        #expect(throws: DecodingError.self) { try decode(json) }
    }

    @Test("the budget is asked for at the team's own path, and a node without it answers 404")
    func asked() async throws {
        let transport = ScriptedTransport([
            ("GET /api/orgs/ac%2Fme/teams/app/budget", HTTPResponse(status: 200, body: try Fixtures.sample("budget"))),
            ("GET /api/orgs/acme/teams/app/budget", json(#"{"error":"not found"}"#, status: 404)),
        ])
        let client = NodeClient(transport: transport, accessToken: { "access-jwt" })
        #expect(try await client.budget(org: "ac/me", team: "app").team.state == .warning)
        await #expect(throws: NodeError.http(status: 404, message: "not found")) { try await client.budget(org: "acme", team: "app") }
    }

    @Test("the capability is named `budget`")
    func capability() {
        #expect(About(capabilities: ["budget"]).has(.budget))
        #expect(!About(capabilities: ["effort"]).has(.budget))
    }
}
