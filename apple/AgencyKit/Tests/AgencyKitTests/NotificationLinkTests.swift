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

    @Test("a recovery payload names the org, and a missing or odd one gives no link")
    func recovery() {
        #expect(NotificationLink(userInfo: payload(["type": "recovery", "org": "acme"])) == .recovery(org: "acme"))
        #expect(NotificationLink(userInfo: payload(["type": "recovery"])) == nil)
        #expect(NotificationLink(userInfo: payload(["type": "recovery", "org": "a b"])) == nil)
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
