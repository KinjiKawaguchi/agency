@testable import AgencyAPI
import Foundation
import Testing

@Suite("Labels on the wire are shown without hidden characters")
struct ShownLabelsTests {
    private let bad = #"E\u202Erin\u200B"#
    private let shown = "E?rin?"

    private func read<T: Decodable>(_ type: T.Type, _ json: String) throws -> T { try JSONDecoder().decode(T.self, from: Data(json.utf8)) }

    @Test("an org, a team, an account and a home node of the account")
    func account() throws {
        let json = #"{"account":{"id":"a","label":"\#(bad)","fingerprint":"f"},"funding":[{"id":"f1","label":"\#(bad)","kind":"apiKey","windows":[]}],"orgs":[{"id":"acme","label":"\#(bad)","isRoot":true,"teams":[{"id":"app","label":"\#(bad)","location":"local"}]}],"homeNodes":[{"label":"\#(bad)","url":"https://work.example.com"}]}"#
        let me = try read(Me.self, json)
        #expect(me.account.label == shown && me.orgs[0].label == shown && me.orgs[0].teams[0].label == shown && me.homeNodes[0].label == shown)
        #expect(me.funding[0].label == shown)
    }

    @Test("a team's view and its participants, a KPI, a device and a push token")
    func others() throws {
        let person = #"{"id":"p","label":"\#(bad)","kind":"human","role":"member","state":"idle","status":"","holding":[]}"#
        let view = try read(TeamView.self, #"{"org":"acme","team":"app","label":"\#(bad)","generatedAt":"2026-11-26T09:00:00Z","people":[\#(person)],"requests":[],"work":[],"epics":[]}"#)
        #expect(view.label == shown && view.people[0].label == shown)
        let kpi = try read(Kpi.self, #"{"id":"k","label":"\#(bad)","kind":"business","unit":"count","goodWhen":"higher"}"#)
        #expect(kpi.label == shown)
        let device = try read(DeviceRequest.self, #"{"label":"\#(bad)","publicKey":"k","deviceId":"d-1","pairingCode":"1234-5678","requestedAt":"2026-11-26T09:00:00Z"}"#)
        #expect(device.label == shown)
        let token = try read(RegisteredPushToken.self, #"{"token":"abcd1234","environment":"production","label":"\#(bad)","registeredAt":"2026-11-26T09:00:00Z"}"#)
        #expect(token.label == shown)
    }

    @Test("what a held root change names, the node's words for it included, and a participant's name")
    func rootChange() throws {
        let json = #"{"account":{"id":"a","label":"A","fingerprint":"f"},"funding":[],"orgs":[],"rootChanges":[{"id":"c1","org":"acme","kind":"addMember","member":"carol","label":"\#(bad)","what":"add member carol (\#(bad))","receivedAt":"2026-11-26T09:00:00Z"}]}"#
        let change = try #require(try read(Me.self, json).rootChanges.first)
        #expect(change.label == shown && change.what == "add member carol (\(shown))")
    }

    @Test("a label that has none of them is read as it was written")
    func plain() throws {
        let me = try read(Me.self, #"{"account":{"id":"a","label":"A \u00e9 営業","fingerprint":"f"},"funding":[],"orgs":[]}"#)
        #expect(me.account.label == "A é 営業")
    }

    @Test("a participant's name in an evaluation")
    func participant() throws {
        var root = try #require(JSONSerialization.jsonObject(with: Fixtures.data("evaluation")) as? [String: Any])
        var teams = try #require(root["teams"] as? [[String: Any]])
        var participants = try #require(teams[0]["participants"] as? [[String: Any]])
        participants[0]["name"] = "Erin\u{202E}"
        teams[0]["participants"] = participants
        root["teams"] = teams
        let evaluation = try JSONDecoder().decode(Evaluation.self, from: JSONSerialization.data(withJSONObject: root))
        #expect(evaluation.teams[0].participants?.first?.name == "Erin?")
    }

    @Test("the names a runtime gives (whom a request is from and to, who wrote a message) and the label of a chosen option in the log")
    func runtimeNames() throws {
        let request = try read(Request.self, #"{"id":"r-1","kind":"question","title":"t","context":"c","from":"\#(bad)","to":"\#(bad)","createdAt":"2026-11-24T11:00:00Z","blocks":[],"options":[],"thread":[{"from":"\#(bad)","text":"x","at":"2026-11-24T11:10:00Z"}],"answerWith":{"kind":"answer","target":"r-1"}}"#)
        #expect(request.from == shown && request.to == shown && request.thread?.first?.from == shown)
        let bare = try read(Request.self, #"{"id":"r-2","kind":"question","title":"t","context":"c","from":"backend","createdAt":"2026-11-24T11:00:00Z","blocks":[],"options":[],"answerWith":{"kind":"answer","target":"r-2"}}"#)
        #expect(bare.to == nil && bare.from == "backend", "an addressee the node leaves out is none")
        let entry = #"{"id":"e1","org":"acme","team":"app","kind":"answer","target":"r-1","text":"","choice":"a","choiceLabel":"\#(bad)","decidedAt":"2026-11-26T09:00:00Z","state":{"kind":"applied"}}"#
        #expect(try read(DecisionEntry.self, entry).choiceLabel == shown)
        #expect(try read(DecisionEntry.self, entry.replacingOccurrences(of: #","choiceLabel":"\#(bad)""#, with: "")).choiceLabel == nil)
        #expect(try read(DecisionEntry.self, entry.replacingOccurrences(of: #""\#(bad)""#, with: "null")).choiceLabel == nil)
    }

    @Test("the node's own text of a reason: a team that could not be read, and why APNs refused a notice")
    func reasons() throws {
        let unavailable = try read(TeamUnavailable.self, #"{"team":"app","reason":"busy\u202E"}"#)
        #expect(unavailable.reason == "busy?")
        let failure = try read(PushFailure.self, #"{"at":"2026-11-26T09:00:00Z","reason":"APNs answered\u200B 503"}"#)
        #expect(failure.reason == "APNs answered? 503")
        #expect(try read(ReportedTime.self, #""later\u202E""#).text == "later?")
        #expect(try read(ReportedTime.self, #""2026-11-26T09:00:00Z""#).date != nil)
    }
}

