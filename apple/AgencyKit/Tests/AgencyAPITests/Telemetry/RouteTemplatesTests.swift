import Foundation
import Testing
@testable import AgencyAPI

@Suite("The routes a client may name")
struct RouteTemplatesTests {
    private func listed(from file: String = #filePath) throws -> [String] {
        // Tests/AgencyAPITests/Telemetry/RouteTemplatesTests.swift: six levels up is the repository root.
        var url = URL(fileURLWithPath: file)
        for _ in 0..<6 { url.deleteLastPathComponent() }
        let data = try Data(contentsOf: url.appendingPathComponent("testdata/route-templates.json"))
        return try JSONDecoder().decode(Listing.self, from: data).routes
    }

    private struct Listing: Decodable { let routes: [String] }

    @Test("the app's table is the node's list, in the same order")
    func sameAsTheFile() throws {
        #expect(RouteTemplates.all == (try listed()))
    }

    @Test("a call is named by its template, whatever its ids and query")
    func templates() {
        let cases: [(String, String, String?)] = [
            ("GET", "/api/me", "GET /api/me"),
            ("GET", "/api/orgs/acme/teams/app/view", "GET /api/orgs/{org}/teams/{team}/view"),
            ("GET", "/api/orgs/acme/teams/app/usage?days=7", "GET /api/orgs/{org}/teams/{team}/usage"),
            ("POST", "/api/orgs/acme/teams/app/envelopes", "POST /api/orgs/{org}/teams/{team}/envelopes"),
            ("GET", "/api/orgs/acme", "GET /api/orgs/{org}"),
            ("GET", "/api/metrics?days=7&hourBuckets=1", "GET /api/metrics"),
            ("DELETE", "/api/push/tokens/ab%2Fcd", "DELETE /api/push/tokens/{token}"),
            ("GET", "/api/orgs/acme/teams/app/budget", "GET /api/orgs/{org}/teams/{team}/budget"),
        ]
        for (method, path, expected) in cases {
            #expect(RouteTemplates.template(method: method, path: path) == expected, Comment(rawValue: "\(method) \(path)"))
        }
    }

    @Test("what is none of the listed routes is not named: another method, an extra or a missing segment, an empty id, the report itself")
    func notListed() {
        let none: [(String, String)] = [
            ("POST", "/api/me"), ("GET", "/api/me/extra"), ("GET", "/api/orgs//view"), ("GET", "/api/orgs/acme/teams/app"),
            ("GET", "/api/orgs/acme/teams//view"), ("POST", "/api/telemetry"), ("POST", "/app/redeem"), ("GET", "/fed/v1/orgs/acme/about"),
            ("GET", "/api/orgs/acme/teams/app/view/"), ("GET", ""), ("GET", "api/me"), ("get", "/api/me"),
        ]
        for (method, path) in none {
            #expect(RouteTemplates.template(method: method, path: path) == nil, Comment(rawValue: "\(method) \(path)"))
        }
    }
}
