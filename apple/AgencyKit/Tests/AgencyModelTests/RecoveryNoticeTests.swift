@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

@Suite("Recovery notice")
struct RecoveryNoticeTests {
    private func orgs(_ extra: String) throws -> [OrgRef] {
        let json = #"{"account":{"id":"alice","label":"A","fingerprint":"f"},"funding":[],"orgs":[\#(extra)]}"#
        return try JSONDecoder().decode(Me.self, from: Data(json.utf8)).orgs
    }

    private func org(_ id: String, label: String = "Acme", recovery: String? = nil) -> String {
        let field = recovery.map { #","recovery":\#($0)"# } ?? ""
        return #"{"id":"\#(id)","label":"\#(label)","isRoot":true,"teams":[]\#(field)}"#
    }

    private let tokyo = TimeZone(identifier: "Asia/Tokyo")!

    @Test("an org with no recovery says none, and an answer from a node that has none of this is read as before")
    func none() throws {
        #expect(RecoveryNotice.all(in: try orgs(org("acme"))).isEmpty)
        #expect(try orgs(org("acme")).first?.recovery == nil)
        #expect(RecoveryNotice.all(in: []).isEmpty)
    }

    @Test("a started recovery is said with the org's name, when it takes effect, and the command that cancels it, with the org's id")
    func started() throws {
        let notices = RecoveryNotice.all(in: try orgs(org("acme", label: "アクミ", recovery: #"{"takesEffectAt":"2026-11-29T09:30:00Z"}"#)))
        let notice = try #require(notices.first)
        #expect(notice.lines(in: tokyo) == ["アクミ のルート鍵の回復が始まりました。", "11月29日 18:30 に有効になります。"])
        #expect(notice.cancelCommand == "agency root-cancel-recovery --org acme")
        #expect(Ja.Recovery.notYours == "心当たりがないときは、ルート鍵のノードで取り消してください:")
    }

    @Test("without a time the sentence about it is left out, and so is a time that cannot be read")
    func noTime() throws {
        for recovery in ["{}", #"{"takesEffectAt":"soon"}"#] {
            let notice = try #require(RecoveryNotice.all(in: try orgs(org("acme", recovery: recovery))).first)
            #expect(notice.lines(in: tokyo) == ["Acme のルート鍵の回復が始まりました。"], Comment(rawValue: recovery))
        }
    }

    @Test("the new root key and the replaced recovery key are said when the node says them, and only then")
    func keys() throws {
        let both = try #require(RecoveryNotice.all(in: try orgs(org("acme", recovery: #"{"newRootKey":"ab12cd34","replacesRecoveryKey":true}"#))).first)
        #expect(both.lines(in: tokyo) == ["Acme のルート鍵の回復が始まりました。", "新しいルート鍵: ab12cd34", "回復用の鍵も取り替えます。"])
        let rootOnly = try #require(RecoveryNotice.all(in: try orgs(org("acme", recovery: #"{"newRootKey":"ab12cd34"}"#))).first)
        #expect(!rootOnly.replacesRecoveryKey && !rootOnly.lines(in: tokyo).contains("回復用の鍵も取り替えます。"))
        let keyOnly = try #require(RecoveryNotice.all(in: try orgs(org("acme", recovery: #"{"replacesRecoveryKey":true}"#))).first)
        #expect(keyOnly.newRootKey == nil && keyOnly.lines(in: tokyo).count == 2)
    }

    @Test("one notice for each org that has a recovery, in the account's order, and none for the others")
    func several() throws {
        let notices = RecoveryNotice.all(in: try orgs([org("zeta", recovery: "{}"), org("acme"), org("lab", recovery: "{}")].joined(separator: ",")))
        #expect(notices.map(\.orgID) == ["zeta", "lab"])
    }

    @Test("a push for a recovery opens the desk, and one with an odd org opens nothing")
    func link() {
        #expect(NotificationLinkProbe.recovery(["agency": ["type": "recovery", "org": "acme"]]) == "acme")
        #expect(NotificationLinkProbe.recovery(["agency": ["type": "recovery"]]) == nil)
        #expect(NotificationLinkProbe.recovery(["agency": ["type": "recovery", "org": "../x"]]) == nil)
    }
}

import AgencyKit

private enum NotificationLinkProbe {
    static func recovery(_ userInfo: [AnyHashable: Any]) -> String? {
        if case .recovery(let org)? = NotificationLink(userInfo: userInfo) { org } else { nil }
    }
}
