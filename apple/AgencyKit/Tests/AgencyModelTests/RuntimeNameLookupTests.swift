import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

@Suite("A runtime name with a hidden character still finds its person")
struct RuntimeNameLookupTests {
    @Test("the name a request carries finds the person who has it, whatever it holds, and is shown without the hidden character")
    func lookup() throws {
        let name = "a\u{200B}b"
        let json = """
        {"view":{"org":"acme","team":"app","label":"App","generatedAt":"2026-11-26T09:00:00Z",
          "people":[{"id":"\(name)","label":"Ann B","kind":"human","role":"member","state":"idle","status":"","holding":[]},
                    {"id":"\(name)2","label":"Carl","kind":"human","role":"member","state":"idle","status":"","holding":[]}],
          "requests":[{"id":"r-1","kind":"question","title":"T","context":"","from":"\(name)","to":"\(name)2","createdAt":"2026-11-26T08:00:00Z",
            "blocks":[],"options":[],"answerWith":{"kind":"answer","target":"r-1"}}],
          "work":[],"epics":[]},
         "viewer":"owner","mine":[],"waitingOnViewer":[],"source":{"kind":"local"}}
        """
        let team = Team(snapshot: try JSONDecoder().decode(TeamSnapshot.self, from: Data(json.utf8)))
        let item = try #require(Desk.openItems([team]).first)
        #expect(item.request.from == name && item.request.shownFrom == "a?b", "kept as written, drawn without the hidden character")
        #expect(item.drafterName == "Ann B" && item.deciderName == "Carl", "a listed person is drawn by label")
        #expect(item.drafter?.label == "Ann B", "found by the name as the runtime wrote it")
        #expect(item.decider?.label == "Carl")
    }

    @Test("a name the team does not list is drawn without the hidden characters, for the drafter, the addressee and a message")
    func unlisted() throws {
        let json = """
        {"view":{"org":"acme","team":"app","label":"App","generatedAt":"2026-11-26T09:00:00Z","people":[],
          "requests":[{"id":"r-1","kind":"question","title":"T","context":"","from":"E\u{202E}rin","to":"Z\u{200B}ed","createdAt":"2026-11-26T08:00:00Z",
            "blocks":[],"options":[],"answerWith":{"kind":"answer","target":"r-1"},"thread":[{"from":"B\u{202E}ob","text":"x","at":"2026-11-26T08:30:00Z"}]}],
          "work":[],"epics":[]},
         "viewer":"owner","mine":[],"waitingOnViewer":[],"source":{"kind":"local"}}
        """
        let team = Team(snapshot: try JSONDecoder().decode(TeamSnapshot.self, from: Data(json.utf8)))
        let item = try #require(Desk.openItems([team]).first)
        #expect(item.drafterName == "E?rin" && item.deciderName == "Z?ed")
        #expect(item.request.thread?.first?.shownFrom == "B?ob")
        #expect(item.threadAuthors == ["B?ob"], "an author the team does not list is drawn by the shown name")
    }

    @Test("a message's author the team lists is drawn by label, as the drafter is, even when the name holds a hidden character")
    func threadAuthorByLabel() throws {
        let name = "a\u{200B}b"
        let json = """
        {"view":{"org":"acme","team":"app","label":"App","generatedAt":"2026-11-26T09:00:00Z",
          "people":[{"id":"\(name)","label":"Ann B","kind":"human","role":"member","state":"idle","status":"","holding":[]}],
          "requests":[{"id":"r-1","kind":"question","title":"T","context":"","from":"\(name)","createdAt":"2026-11-26T08:00:00Z",
            "blocks":[],"options":[],"answerWith":{"kind":"answer","target":"r-1"},
            "thread":[{"from":"\(name)","text":"x","at":"2026-11-26T08:30:00Z"},{"from":"Zed","text":"y","at":"2026-11-26T08:40:00Z"}]}],
          "work":[],"epics":[]},
         "viewer":"owner","mine":[],"waitingOnViewer":[],"source":{"kind":"local"}}
        """
        let team = Team(snapshot: try JSONDecoder().decode(TeamSnapshot.self, from: Data(json.utf8)))
        let item = try #require(Desk.openItems([team]).first)
        #expect(item.threadAuthors == ["Ann B", "Zed"], "listed: the label; not listed: the name")
        #expect(item.drafterName == "Ann B")
    }
}
