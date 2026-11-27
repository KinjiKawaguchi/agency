import AgencyAPI
@testable import AgencyModel
import AgencyProtocol
import Foundation
import Testing

@Suite("Desk replies")
struct DeskReplyTests {
    private let team = Teams.team()

    private func item(_ id: String, in team: Team? = nil) throws -> DeskItem {
        try #require(Desk.openItems([team ?? self.team]).first { $0.request.id == id })
    }

    // MARK: Choosing

    @Test("choosing an option decides the request, with the option's label for the log")
    func choose() throws {
        let item = try item(Teams.questionID)
        let decision = try item.prepare(.choose(optionKey: "b", note: "  Because it is boring.  "))
        #expect(decision.choice == "b")
        #expect(decision.text == "Because it is boring.")
        #expect(decision.choiceLabel == item.request.options[1].text)
        #expect(!decision.isComment)
        #expect(decision.answerWith == AnswerWith(kind: .answer, target: Teams.questionID))
        #expect(decision.org == "acme" && decision.team == "app")
        #expect(decision.title == "Which database should the app use in production?")
    }

    @Test("a choice needs no note")
    func chooseWithoutNote() throws {
        let decision = try item(Teams.proposalID).prepare(.choose(optionKey: "approve", note: "   "))
        #expect(decision.text == "")
        #expect(decision.choice == "approve")
    }

    @Test("an option the request does not have is refused")
    func unknownOption() throws {
        let item = try item(Teams.questionID)
        #expect(throws: DeskReplyError.unknownOption("zzz")) { try item.prepare(.choose(optionKey: "zzz", note: "")) }
    }

    // MARK: Commenting

    @Test("a comment is text with no choice, and says nothing is decided")
    func comment() throws {
        let decision = try item(Teams.proposalID).prepare(.comment("  来週にしてください  "))
        #expect(decision.isComment)
        #expect(decision.choice == nil)
        #expect(decision.choiceLabel == nil)
        #expect(decision.text == "来週にしてください")
    }

    @Test("a comment needs words")
    func commentNeedsText() throws {
        let item = try item(Teams.proposalID)
        #expect(throws: DeskReplyError.textRequired) { try item.prepare(.comment("")) }
        #expect(throws: DeskReplyError.textRequired) { try item.prepare(.comment(" \n\u{3000}")) }
    }

    @Test("a comment is only for a request that has options; one without takes an answer")
    func commentOnlyWithOptions() throws {
        let decision = try item(Teams.decisionID)
        #expect(throws: DeskReplyError.noOptionsToChoose) { try decision.prepare(.comment("x")) }
        #expect(throws: DeskReplyError.noOptionsToChoose) { try decision.prepare(.choose(optionKey: "a", note: "")) }
        let answer = try decision.prepare(.answer("Yes, drop it."))
        #expect(answer.choice == nil)
        #expect(!answer.isComment)
        #expect(answer.answerWith == AnswerWith(kind: .judge, target: "app#14"))
    }

    @Test("a request with options is not answered in words alone: that is a comment")
    func answerNeedsNoOptions() throws {
        let item = try item(Teams.questionID)
        #expect(throws: DeskReplyError.optionsNeedAChoiceOrAComment) { try item.prepare(.answer("x")) }
    }

    @Test("an answer needs words")
    func answerNeedsText() throws {
        let item = try item(Teams.decisionID)
        #expect(throws: DeskReplyError.textRequired) { try item.prepare(.answer("  ")) }
    }

    @Test("only the viewer's own requests can be answered")
    func notYours() throws {
        let others = Teams.team(addRequests: [Teams.request(to: "frontend")])
        let item = try item("r-20261124-120000-e5f6", in: others)
        #expect(!item.isMine)
        #expect(throws: DeskReplyError.notYours) { try item.prepare(.choose(optionKey: "yes", note: "")) }
        #expect(throws: DeskReplyError.notYours) { try item.prepare(.comment("hm")) }
    }

    // MARK: The envelope

    @Test("the envelope is built from the request's own answerWith, and a comment's has no choice")
    func envelope() throws {
        let id = "00112233445566778899aabbccddeeff"
        let comment = try item(Teams.proposalID).prepare(.comment("もう少し待って 🙏"))
        let envelope = comment.envelope(id: id, account: "alice", at: "2026-11-24T12:00:00Z")
        try envelope.validate()
        #expect(envelope.kind == .answer && envelope.target == Teams.proposalID)
        #expect(envelope.choice == nil)
        #expect(!(try CanonicalJSON.encode(envelope.json)).contains("choice"))

        let judged = try item(Teams.decisionID).prepare(.answer("Drop it."))
        let judgement = judged.envelope(id: id, account: "alice", at: "2026-11-24T12:00:00Z")
        try judgement.validate()
        #expect(judgement.kind == .judge && judgement.target == "app#14")

        let chosen = try item(Teams.questionID).prepare(.choose(optionKey: "a", note: ""))
        try chosen.envelope(id: id, account: "alice", at: "2026-11-24T12:00:00Z").validate()
    }

    // MARK: Notes for the log

    @Test("a title or label of 200 characters is kept; one of 201 is cut to 200 and ends in an ellipsis")
    func clipping() throws {
        let exact = String(repeating: "a", count: 200)
        let long = String(repeating: "a", count: 201)
        #expect(PreparedDecision.clipped(exact) == exact)
        let clipped = PreparedDecision.clipped(long)
        #expect(clipped.unicodeScalars.count == 200)
        #expect(clipped.hasSuffix("…"))
        #expect(clipped.hasPrefix(String(repeating: "a", count: 199)))
    }

    @Test("characters are counted as Unicode scalars, as the node does: an emoji outside the BMP is one, a letter with an accent is two")
    func clippingCountsScalars() {
        // 100 letters with a combining accent are 100 graphemes and 200 scalars: kept whole.
        let accents = String(repeating: "e\u{301}", count: 100)
        #expect(accents.count == 100 && accents.unicodeScalars.count == 200)
        #expect(PreparedDecision.clipped(accents) == accents)
        // One more makes 202 scalars (101 graphemes): cut to 199 scalars and an ellipsis.
        let more = accents + "e\u{301}"
        let clipped = PreparedDecision.clipped(more)
        #expect(clipped.unicodeScalars.count == 200)
        // Emoji are one scalar each (two UTF-16 units): 200 are kept, 201 are cut.
        let emoji = String(repeating: "🙂", count: 200)
        #expect(emoji.utf16.count == 400)
        #expect(PreparedDecision.clipped(emoji) == emoji)
        #expect(PreparedDecision.clipped(emoji + "🙂").unicodeScalars.count == 200)
    }

    @Test("the cut keeps 199 scalars, drops the space it ends on, and adds the ellipsis, as the web client does")
    func clippingTrimsTheEnd() {
        let text = String(repeating: "a", count: 198) + " " + "bcd"
        #expect(PreparedDecision.clipped(text) == String(repeating: "a", count: 198) + "…")
        let wide = String(repeating: "a", count: 197) + "\u{3000} " + "zz"
        #expect(PreparedDecision.clipped(wide) == String(repeating: "a", count: 197) + "…")
    }

    @Test("a note is trimmed before it is counted")
    func clippingTrimsFirst() {
        let padded = "  " + String(repeating: "a", count: 200) + "\n "
        #expect(PreparedDecision.clipped(padded) == String(repeating: "a", count: 200))
    }

    @Test("a cut never splits an emoji or leaves half of a sequence in the title that the node would refuse")
    func clippingKeepsScalarsWhole() {
        let title = String(repeating: "あ", count: 150) + String(repeating: "👩\u{200D}💻", count: 20)
        let clipped = PreparedDecision.clipped(title)
        #expect(clipped.unicodeScalars.count <= 200)
        #expect(String(decoding: Array(clipped.utf8), as: UTF8.self) == clipped)
    }

    @Test("a long request title and a long option label are clipped in the decision, not refused")
    func longNotes() throws {
        let long = String(repeating: "題", count: 250)
        let team = Teams.team(addRequests: [Teams.request(id: "r-long", to: nil, title: long)])
        let item = try item("r-long", in: team)
        let decision = try item.prepare(.choose(optionKey: "yes", note: ""))
        #expect(decision.title.unicodeScalars.count == 200)
    }

    @Test("an option that says nothing sends no label")
    func emptyLabel() throws {
        let team = Teams.team(addRequests: [Teams.request(id: "r-blank", to: nil, optionText: "  ")])
        #expect(try item("r-blank", in: team).prepare(.choose(optionKey: "yes", note: "")).choiceLabel == nil)
    }

    @Test("a long option label is clipped for the log too")
    func longLabel() throws {
        let team = Teams.team(addRequests: [Teams.request(id: "r-label", to: nil, optionText: String(repeating: "選", count: 250))])
        let decision = try item("r-label", in: team).prepare(.choose(optionKey: "yes", note: ""))
        #expect(decision.choiceLabel?.unicodeScalars.count == 200)
    }

    // MARK: Drafts

    @Test("a draft with a pick decides; the text is its note")
    func draftDecides() throws {
        let item = try item(Teams.questionID)
        #expect(DeskDraft(text: "note", optionKey: "a").decision(for: item) == .choose(optionKey: "a", note: "note"))
    }

    @Test("a draft with text and no pick is not a decision when there are options")
    func draftWithoutPick() throws {
        let item = try item(Teams.questionID)
        #expect(DeskDraft(text: "just words").decision(for: item) == nil)
        #expect(DeskDraft().decision(for: item) == nil)
    }

    @Test("a draft for a request with no options is the answer")
    func draftAnswers() throws {
        #expect(DeskDraft(text: "yes").decision(for: try item(Teams.decisionID)) == .answer("yes"))
    }

    @Test("a draft is empty without text and pick, and survives being stored")
    func draftStorage() throws {
        #expect(DeskDraft().isEmpty)
        #expect(!DeskDraft(text: "x").isEmpty && !DeskDraft(optionKey: "a").isEmpty)
        let draft = DeskDraft(text: "来週 🙏", optionKey: "b")
        let stored = try JSONEncoder().encode(draft)
        #expect(try JSONDecoder().decode(DeskDraft.self, from: stored) == draft)
    }
}

@Suite("Requests of a kind the app does not know")
struct UnknownKindRequestTests {
    private func request(kind: String, answerKind: String) -> [String: Any] {
        [
            "id": "r-future", "kind": kind, "title": "Vote on the logo", "context": "c", "from": "backend", "createdAt": "2026-11-24T12:00:00Z",
            "blocks": [], "options": [], "answerWith": ["kind": answerKind, "target": "r-future"],
        ]
    }

    private func item(kind: String = "vote", answerKind: String = "answer") throws -> DeskItem {
        let team = Teams.team(addRequests: [request(kind: kind, answerKind: answerKind)])
        return try #require(Desk.openItems([team]).first { $0.request.id == "r-future" })
    }

    @Test("it is on the desk, counted, in a group of its own, and named by a plain word")
    func shown() throws {
        let item = try item()
        #expect(item.request.kind == .unknown && item.isMine && item.group == .unrecognized)
        #expect(Ja.Desk.kind(.unknown) == "依頼" && Ja.Desk.group(.unrecognized) == "このアプリが知らない種類の依頼")
    }

    @Test("when the node says how to answer it in a way the app knows, it is answered as any request is")
    func answerable() throws {
        let item = try item()
        #expect(item.isAnswerableHere)
        let decision = try item.prepare(.answer("The blue one"))
        #expect(decision.answerWith == AnswerWith(kind: .answer, target: "r-future") && decision.text == "The blue one")
    }

    @Test("when the way to answer it is one the app cannot make, nothing is offered and nothing is sent")
    func notAnswerable() throws {
        let item = try item(answerKind: "tally")
        #expect(!item.isAnswerableHere)
        #expect(throws: DeskReplyError.cannotAnswerHere) { try item.prepare(.answer("x")) }
        let controls = ReplyControls(item: item, draft: DeskDraft(text: "x", optionKey: nil), isSending: false, isDeviceReady: true)
        #expect(!controls.canDecide && !controls.canPick)
    }
}

