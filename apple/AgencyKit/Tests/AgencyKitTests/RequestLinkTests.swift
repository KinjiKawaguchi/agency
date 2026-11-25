import Foundation
import Testing
@testable import AgencyKit

@Suite("RequestLink")
struct RequestLinkTests {
    private let homeNode = try! HomeNodeURL(validating: "https://node.example.com")

    private func payload(org: Any? = "acme", team: Any? = "app", request: Any? = "r-20261124-110000-a1b2") -> [AnyHashable: Any] {
        var keys: [String: Any] = [:]
        if let org { keys["org"] = org }
        if let team { keys["team"] = team }
        if let request { keys["request"] = request }
        return ["aps": ["alert": "x"], "agency": keys]
    }

    @Test("a payload becomes the desk address with the request selected")
    func opensTheRequest() throws {
        let link = try #require(RequestLink(userInfo: payload()))
        let url = link.url(on: homeNode)
        let components = try #require(URLComponents(url: url, resolvingAgainstBaseURL: false))
        #expect(components.host == "node.example.com")
        #expect(components.path == "/desk")
        #expect(components.queryItems?.count == 1)
        #expect(components.queryItems?.first?.name == "item")
        #expect(components.queryItems?.first?.value == "acme:app:r-20261124-110000-a1b2")
    }

    @Test("the address stays on the home node, whatever the payload says")
    func staysOnTheHomeNode() throws {
        let link = try #require(RequestLink(userInfo: payload()))
        #expect(homeNode.hasSameOrigin(link.url(on: homeNode)))
        let local = try HomeNodeURL(validating: "http://localhost:3000")
        #expect(local.hasSameOrigin(link.url(on: local)))
    }

    @Test("a payload that is missing a value or has an odd one gives no link", arguments: [
        ["org": nil, "team": "app", "request": "r-1"],
        ["org": "acme", "team": nil, "request": "r-1"],
        ["org": "acme", "team": "app", "request": nil],
        ["org": "", "team": "app", "request": "r-1"],
        ["org": "ac:me", "team": "app", "request": "r-1"],
        ["org": "acme", "team": "app", "request": "r 1"],
        ["org": "acme", "team": "app", "request": "r-1&item=x"],
        ["org": "acme", "team": "app", "request": "../r-1"],
        ["org": "acme", "team": "app", "request": "https://evil.example"],
        ["org": "acme", "team": "app", "request": String(repeating: "a", count: 129)],
    ] as [[String: String?]])
    func rejects(values: [String: String?]) {
        let info = payload(org: values["org"] ?? nil, team: values["team"] ?? nil, request: values["request"] ?? nil)
        #expect(RequestLink(userInfo: info) == nil)
    }

    @Test("values that are not strings, and a payload without the agency keys, give no link")
    func rejectsOtherShapes() {
        #expect(RequestLink(userInfo: payload(request: 42)) == nil)
        #expect(RequestLink(userInfo: ["aps": ["alert": "x"]]) == nil)
        #expect(RequestLink(userInfo: ["agency": "acme"]) == nil)
    }
}
