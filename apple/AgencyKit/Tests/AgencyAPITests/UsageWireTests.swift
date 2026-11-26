import Foundation
import Testing
@testable import AgencyAPI

/// Answers every request with one fixed response and remembers the request.
private final class OneAnswerTransport: HTTPTransport, @unchecked Sendable {
    private let lock = NSLock()
    private var sent: [HTTPRequest] = []
    private let response: HTTPResponse

    init(_ response: HTTPResponse) { self.response = response }

    var requests: [HTTPRequest] { lock.withLock { sent } }

    func send(_ request: HTTPRequest) async throws -> HTTPResponse {
        lock.withLock { sent.append(request) }
        return response
    }
}

@Suite("Token usage")
struct UsageWireTests {
    @Test("the usage answer decodes: days, the cost of each day by participant, and each participant's tokens")
    func decodes() throws {
        let usage = try JSONDecoder().decode(TokenUsage.self, from: Fixtures.data("usage"))
        #expect(usage.org == "acme")
        #expect(usage.team == "app")
        #expect(usage.days == 7)
        #expect(usage.byDay.map(\.date) == ["2026-11-23", "2026-11-24"])
        #expect(usage.byDay[1].costUsd == ["backend": 2.05, "lead": 1.2, "reviewer": 0.5])
        #expect(usage.byPerson.map(\.person) == ["backend", "lead", "reviewer"])
        #expect(usage.byPerson[0].costUsd == 8.45)
        #expect(usage.byPerson[0].tokens == TokenCounts(input: 120_000, output: 45_000, cacheRead: 2_400_000, cacheWrite: 180_000))
    }

    @Test("the client asks for the team's usage over a number of days, with the ids escaped")
    func request() async throws {
        let transport = OneAnswerTransport(HTTPResponse(status: 200, body: try Fixtures.data("usage")))
        let client = NodeClient(transport: transport, accessToken: { "access-jwt" })
        let usage = try await client.usage(org: "a/b", team: "app", days: 7)
        #expect(usage.days == 7)
        #expect(transport.requests.map(\.path) == ["/api/orgs/a%2Fb/teams/app/usage?days=7"])
        #expect(transport.requests[0].headers[NodeClient.accessHeader] == "access-jwt")
    }

    @Test("a team that does not report usage answers 404, which the client passes on as it is")
    func notReported() async {
        let body = Data(#"{"error":"no usage"}"#.utf8)
        let client = NodeClient(transport: OneAnswerTransport(HTTPResponse(status: 404, body: body)), accessToken: { "t" })
        await #expect(throws: NodeError.http(status: 404, message: "no usage")) {
            try await client.usage(org: "acme", team: "app", days: 7)
        }
    }
}
