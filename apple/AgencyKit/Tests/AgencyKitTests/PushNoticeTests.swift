@testable import AgencyKit
import Foundation
import Testing

@Suite("Push notice words")
struct PushNoticeTests {
    private let tokyo = TimeZone(identifier: "Asia/Tokyo")!
    private let newYork = TimeZone(identifier: "America/New_York")!

    private func payload(_ keys: [String: Any]) -> [AnyHashable: Any] { ["agency": keys] }

    private func body(_ keys: [String: Any], _ language: String = "ja-JP", _ zone: TimeZone? = nil) -> String? {
        PushNotice.body(userInfo: payload(keys), language: language, timeZone: zone ?? tokyo)
    }

    @Test("a recovery says the org, when it was started and when it takes effect, in the viewer's zone")
    func recoveryWithTimes() {
        let keys: [String: Any] = ["type": "recovery", "org": "acme", "startedAt": "2026-11-26T09:30:00Z", "finishesAt": "2026-11-29T09:30:00Z"]
        #expect(body(keys) == "acme のルートキーの回復が始まりました（11月26日 18:30）。11月29日 18:30 に有効になります。机で確認してください。")
        #expect(body(keys, "ja", newYork) == "acme のルートキーの回復が始まりました（11月26日 4:30）。11月29日 4:30 に有効になります。机で確認してください。")
        #expect(body(keys, "en-US", newYork) == "A recovery of acme was started at Nov 26, 4:30 AM. It takes effect at Nov 29, 4:30 AM. Open the desk to see it.")
    }

    @Test("a recovery with no time, or one that is not a time, says only that it started; with no org it says of an org")
    func recoveryWithout() {
        #expect(body(["type": "recovery", "org": "acme"]) == "acme のルートキーの回復が始まりました。机で確認してください。")
        #expect(body(["type": "recovery", "org": "acme", "startedAt": "yesterday", "finishesAt": 7]) == "acme のルートキーの回復が始まりました。机で確認してください。")
        #expect(body(["type": "recovery"]) == "組織のルートキーの回復が始まりました。机で確認してください。")
        #expect(body(["type": "recovery", "org": "a b"], "en") == "A recovery of an org was started. Open the desk to see it.", "an org that is not an identifier is not named")
        #expect(body(["type": "recovery", "org": "acme", "finishesAt": "2026-11-29T09:30:00.250Z"], "en", tokyo) == "A recovery of acme was started. It takes effect at Nov 29, 6:30 PM. Open the desk to see it.")
    }

    @Test("a held root change says one waits, or how many, and names the org when there is one")
    func root() {
        #expect(body(["type": "root", "org": "acme", "id": "h-1", "waiting": 1]) == "acme の変更が、ルートキーの署名を待っています。机で確認してください。")
        #expect(body(["type": "root", "org": "acme", "id": "h-1", "waiting": 3]) == "acme の変更 3 件が、ルートキーの署名を待っています。机で確認してください。")
        #expect(body(["type": "root", "waiting": 2]) == "変更 2 件が、ルートキーの署名を待っています。机で確認してください。")
        #expect(body(["type": "root", "org": "acme", "waiting": 1], "en") == "A change to acme waits for the root key’s signature. Open the desk to see it.")
        #expect(body(["type": "root", "org": "acme", "waiting": 3], "en-GB") == "3 changes to acme wait for the root key’s signature. Open the desk to see them.")
    }

    @Test("a count that is not a whole number counts one change, and what the node's own words name is never used")
    func odd() {
        #expect(body(["type": "root", "org": "acme", "waiting": "many"]) == "acme の変更が、ルートキーの署名を待っています。机で確認してください。")
        #expect(body(["type": "root", "org": "acme"], "en") == "A change to acme waits for the root key’s signature. Open the desk to see it.")
        let withLabel: [AnyHashable: Any] = [
            "agency": ["type": "root", "org": "acme", "waiting": 1], "aps": ["alert": ["title": "x", "body": "add member carol (Carol). Run agency root-sign"]],
        ]
        #expect(PushNotice.body(userInfo: withLabel, language: "en", timeZone: tokyo)?.contains("Carol") == false)
    }

    @Test("the other kinds, a payload that is not ours, and a type the app does not know get no words from here")
    func others() {
        for keys in [["org": "acme", "team": "app", "request": "r-1"], ["type": "device"], ["type": "decision", "org": "acme", "team": "app", "id": "d-1"], ["type": "future"]] as [[String: Any]] {
            #expect(body(keys) == nil, Comment(rawValue: "\(keys)"))
        }
        #expect(PushNotice.body(userInfo: ["aps": ["alert": "x"]], language: "ja", timeZone: tokyo) == nil)
        #expect(PushNotice.title == "Agency")
    }
}
