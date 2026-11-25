import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

@Suite("ReplyControls")
struct ReplyControlsTests {
    private let team = Teams.team()

    private func item(_ id: String, in team: Team? = nil) throws -> DeskItem {
        try #require(Desk.openItems([team ?? self.team]).first { $0.request.id == id })
    }

    private func controls(_ item: DeskItem, _ draft: DeskDraft = DeskDraft(), sending: Bool = false, sent: Bool = false) -> ReplyControls {
        ReplyControls(item: item, draft: draft, isSending: sending, isSent: sent)
    }

    @Test("with options: nothing to decide until one is picked; a note alone can only be a comment")
    func withOptions() throws {
        let question = try item(Teams.questionID)
        let none = controls(question)
        #expect(none.canPick && !none.canDecide && none.showsComment && !none.canComment)
        let note = controls(question, DeskDraft(text: "待って"))
        #expect(!note.canDecide && note.canComment)
        let picked = controls(question, DeskDraft(optionKey: "a"))
        #expect(picked.canDecide && !picked.canComment)
        let both = controls(question, DeskDraft(text: "条件つき", optionKey: "a"))
        #expect(both.canDecide && both.canComment)
    }

    @Test("a note of spaces is no note")
    func blankNote() throws {
        let question = try item(Teams.questionID)
        #expect(!controls(question, DeskDraft(text: " \n\u{3000}")).canComment)
    }

    @Test("a pick that is not one of the request's options does not enable deciding")
    func foreignPick() throws {
        #expect(!controls(try item(Teams.questionID), DeskDraft(optionKey: "zzz")).canDecide)
    }

    @Test("without options: words answer the request and there is no comment button")
    func withoutOptions() throws {
        let decision = try item(Teams.decisionID)
        #expect(!controls(decision).canDecide)
        let typed = controls(decision, DeskDraft(text: "yes"))
        #expect(typed.canDecide && !typed.showsComment && !typed.canComment)
    }

    @Test("the main button says the verb of the request, and with an option picked, which one")
    func label() throws {
        let question = try item(Teams.questionID)
        #expect(controls(question).decideLabel == "答える")
        let picked = controls(question, DeskDraft(optionKey: "b"))
        #expect(picked.decideLabel == "「\(question.request.options[1].text)」で答える")
        #expect(controls(try item(Teams.proposalID)).decideLabel == "決裁する")
        #expect(controls(try item(Teams.decisionID)).decideLabel == "判断を送る")
    }

    @Test("nothing can be pressed while sending, after sending, or on a request for someone else")
    func locked() throws {
        let question = try item(Teams.questionID)
        let draft = DeskDraft(text: "x", optionKey: "a")
        for locked in [controls(question, draft, sending: true), controls(question, draft, sent: true)] {
            #expect(!locked.canPick && !locked.canDecide && !locked.canComment)
        }
        let others = Teams.team(addRequests: [Teams.request(to: "frontend")])
        let theirs = try item("r-20261124-120000-e5f6", in: others)
        let none = controls(theirs, DeskDraft(text: "x", optionKey: "yes"))
        #expect(!none.canPick && !none.canDecide && !none.canComment)
    }
}
