@testable import AgencyAPI
@testable import AgencyModel
import AgencyProtocol
import Foundation
import Testing

@Suite("Root change notice")
struct RootChangeNoticeTests {
    private let now = Timestamp.date(from: "2026-11-26T12:00:00Z")!

    private func me(_ changes: [String], orgs: String = #"{"id":"acme","label":"アクミ","isRoot":true,"teams":[]}"#) throws -> Me {
        let json = #"{"account":{"id":"alice","label":"A","fingerprint":"f"},"funding":[],"orgs":[\#(orgs)],"rootChanges":[\#(changes.joined(separator: ","))]}"#
        return try JSONDecoder().decode(Me.self, from: Data(json.utf8))
    }

    private func change(_ fields: String, id: String = "c1", org: String = "acme", at: String = "2026-11-26T09:00:00Z") -> String {
        #"{"id":"\#(id)","org":"\#(org)","receivedAt":"\#(at)"\#(fields)}"#
    }

    @Test("the answer of #381 as the node writes it (the shared fixture) is worded from its kinds, in the order the node gave")
    func theNodesAnswer() throws {
        let notice = try #require(RootChangeNotice(me: Fixture.decode(Me.self, "me-root-changes")))
        #expect(notice.items.map(\.what) == ["組織のメンバー Carol（carol）の追加", "bob の端末「iPhone」の追加", "bob の端末 d-dbc298251c51 の削除"])
        #expect(notice.items.map(\.org) == ["Acme", "Acme", "Acme"])
        #expect(Fixture.decode(Me.self, "me-root-changes").rootChanges.compactMap(\.device) == ["d-a652124afda7", "d-dbc298251c51"])
    }

    @Test("nothing is said when nothing waits")
    func none() throws {
        #expect(RootChangeNotice(me: try me([])) == nil)
    }

    @Test("each kind the app knows is worded from its fields, in the web's words")
    func kinds() throws {
        let notice = try #require(RootChangeNotice(me: try me([
            change(#","what":"add member carol (Carol)","kind":"addMember","member":"carol","label":"Carol""#, id: "a"),
            change(#","what":"add device iPhone for bob","kind":"addDevice","member":"bob","label":"iPhone""#, id: "b"),
            change(#","what":"remove device d-1 of bob","kind":"removeDevice","member":"bob","device":"d-1""#, id: "c"),
        ])))
        #expect(notice.items.map(\.what) == ["組織のメンバー Carol（carol）の追加", "bob の端末「iPhone」の追加", "bob の端末 d-1 の削除"])
    }

    @Test("the node's own words are said for a kind the app does not know, and for a known kind that lacks a field")
    func fallback() throws {
        let notice = try #require(RootChangeNotice(me: try me([
            change(#","what":"rotate the moon","kind":"rotateMoon","member":"bob""#, id: "a"),
            change(#","what":"add member carol (Carol)","kind":"addMember","member":"carol""#, id: "b"),
            change(#","what":"remove device d-1 of bob","kind":"removeDevice","device":"d-1""#, id: "c"),
            change(#","what":"add device iPhone for bob""#, id: "d"),
        ])))
        #expect(notice.items.map(\.what) == ["rotate the moon", "add member carol (Carol)", "remove device d-1 of bob", "add device iPhone for bob"])
    }

    @Test("a change with neither words nor fields to word it from is left out, and a list of only those says nothing")
    func wordless() throws {
        let kept = try #require(RootChangeNotice(me: try me([change(#","kind":"addMember","member":"carol""#, id: "a"), change(#","what":"x""#, id: "b")])))
        #expect(kept.items.map(\.id) == ["b"])
        #expect(RootChangeNotice(me: try me([change(#","kind":"addMember","member":"carol""#)])) == nil)
    }

    @Test("the title counts what waits, and each line gives the org's label and how long ago it was asked")
    func lines() throws {
        let notice = try #require(RootChangeNotice(me: try me([change(#","what":"add member carol (Carol)""#, id: "a"), change(#","what":"add device iPhone for bob""#, id: "b", at: "2026-11-24T09:00:00Z")])))
        #expect(notice.title == "組織のルートキーの署名を待っている変更が 2 件あります。")
        #expect(notice.lines(now: now) == ["add member carol (Carol)（アクミ、3 時間前にリクエスト）", "add device iPhone for bob（アクミ、2 日前にリクエスト）"])
    }

    @Test("an org the account does not list is named by its id, and a time that cannot be read is left out")
    func orgAndTime() throws {
        let notice = try #require(RootChangeNotice(me: try me([change(#","what":"x""#, org: "other", at: "yesterday")])))
        #expect(notice.lines(now: now) == ["x（other）"])
    }

    @Test("the command is the app's own word, in the sentence the web says, whatever the node's changes say")
    func command() throws {
        let notice = try #require(RootChangeNotice(me: try me([change(#","what":"agency root-sign --everything""#)])))
        let parts = notice.signTogether
        #expect(parts.before + parts.command + parts.after == "まとめて署名するには、組織のルートキーのノードで agency root-sign を実行してください。")
        #expect(RootChangeNotice.approvedHere == "ここでは承認しました。組織のルートキーのノードが署名すると有効になります。そのノードで agency root-sign を実行してください。")
    }
}
