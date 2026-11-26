import Foundation
import Testing
@testable import AgencyKit

@Suite("NotificationLink")
struct NotificationLinkTests {
    private func payload(_ keys: [String: Any]) -> [AnyHashable: Any] { ["aps": ["alert": "x"], "agency": keys] }

    @Test("a payload without a type is a request")
    func request() throws {
        let link = NotificationLink(userInfo: payload(["org": "acme", "team": "app", "request": "r-1"]))
        #expect(link == .request(try #require(RequestLink(userInfo: payload(["org": "acme", "team": "app", "request": "r-1"])))))
    }

    @Test("a device payload opens the devices screen")
    func device() {
        #expect(NotificationLink(userInfo: payload(["type": "device"])) == .devices)
    }

    @Test("a decision payload names the org, the team and the decision")
    func decision() {
        #expect(NotificationLink(userInfo: payload(["type": "decision", "org": "acme", "team": "app", "id": "dc-1"])) == .decision(org: "acme", team: "app", id: "dc-1"))
    }

    @Test("a decision payload with a missing or odd value gives no link", arguments: [
        ["type": "decision", "team": "app", "id": "dc-1"],
        ["type": "decision", "org": "acme", "id": "dc-1"],
        ["type": "decision", "org": "acme", "team": "app"],
        ["type": "decision", "org": "ac me", "team": "app", "id": "dc-1"],
        ["type": "decision", "org": "acme", "team": "app", "id": "dc-1/../x"],
        ["type": "decision", "org": "acme", "team": "app", "id": ""],
    ] as [[String: String]])
    func badDecision(keys: [String: String]) {
        #expect(NotificationLink(userInfo: payload(keys)) == nil)
    }

    @Test("a type this app does not know, a type that is not text, and a payload that is not ours give no link")
    func unknown() {
        #expect(NotificationLink(userInfo: payload(["type": "invoice", "org": "acme", "team": "app", "request": "r-1"])) == nil)
        #expect(NotificationLink(userInfo: payload(["type": 7, "org": "acme", "team": "app", "request": "r-1"])) == nil)
        #expect(NotificationLink(userInfo: ["aps": ["alert": "x"]]) == nil)
        #expect(NotificationLink(userInfo: payload(["org": "acme", "team": "app"])) == nil)
    }
}

@Suite("NotificationTap")
struct NotificationTapTests {
    private let id = "02ae509c6b20999646d3c600687ee1bf"
    private func payload(_ keys: [String: Any]) -> [AnyHashable: Any] { ["aps": ["alert": "x"], "agency": keys] }

    @Test("a push names the node that sent it, for every kind")
    func names() {
        for keys: [String: Any] in [
            ["org": "acme", "team": "app", "request": "r-1", "node": id],
            ["type": "device", "node": id],
            ["type": "decision", "org": "acme", "team": "app", "id": "dc-1", "node": id],
        ] {
            #expect(NotificationTap(userInfo: payload(keys))?.node == id, Comment(rawValue: "\(keys)"))
        }
    }

    @Test("a push with no node, or one that is not an id, names none, and its link is unchanged")
    func none() throws {
        for node: Any? in [nil, "", "work.example.com", "02AE509C6B20999646D3C600687EE1BF", 7, ["a": 1]] {
            var keys: [String: Any] = ["type": "device"]
            if let node { keys["node"] = node }
            let tap = try #require(NotificationTap(userInfo: payload(keys)))
            #expect(tap.node == nil)
            #expect(tap.link == .devices)
        }
    }

    @Test("a payload with no link gives no tap, with or without a node")
    func noLink() {
        #expect(NotificationTap(userInfo: payload(["type": "nonsense", "node": id])) == nil)
        #expect(NotificationTap(userInfo: ["aps": ["alert": "x"]]) == nil)
    }
}
