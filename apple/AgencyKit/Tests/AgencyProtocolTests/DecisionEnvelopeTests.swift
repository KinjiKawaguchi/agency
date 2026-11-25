import Foundation
import Testing
@testable import AgencyProtocol

@Suite("DecisionEnvelope")
struct DecisionEnvelopeTests {
    private func make(
        id: String = "00112233445566778899aabbccddeeff", kind: DecisionEnvelope.Kind = .answer,
        org: String = "acme", team: String = "app", target: String = "r-20260101-000000-abcd",
        text: String = "", choice: String? = "a", account: String = "alice", at: String = "2026-11-24T12:00:00Z"
    ) -> DecisionEnvelope {
        DecisionEnvelope(id: id, kind: kind, org: org, team: team, target: target, text: text,
                         choice: choice, account: account, at: at)
    }

    @Test("a choice alone, a choice with a note, and a comment alone are valid")
    func valid() throws {
        try make().validate()
        try make(text: "Go ahead.", choice: "a").validate()
        try make(text: "Please add the migration first.", choice: nil).validate()
        try make(kind: .judge, target: "acme-app#12").validate()
    }

    @Test("a comment is an answer with text and no choice")
    func commentJSON() throws {
        let comment = make(text: "もう少し待ってください", choice: nil)
        #expect(try CanonicalJSON.encode(comment.json).contains("\"payload\":{\"text\":\"もう少し待ってください\"}"))
        #expect(try !CanonicalJSON.encode(comment.json).contains("choice"))
    }

    @Test("nothing to say is refused: no choice and a text of spaces")
    func empty() {
        #expect(throws: EnvelopeError.empty) { try make(text: "", choice: nil).validate() }
        #expect(throws: EnvelopeError.empty) { try make(text: " \n\u{3000}", choice: nil).validate() }
    }

    @Test("the text is at most 4000 characters, counted as Unicode scalars")
    func textLength() throws {
        try make(text: String(repeating: "あ", count: 4000)).validate()
        #expect(throws: EnvelopeError.textTooLong) { try make(text: String(repeating: "あ", count: 4001)).validate() }
        #expect(throws: EnvelopeError.textTooLong) { try make(text: String(repeating: "🙂", count: 4001)).validate() }
        // 2001 characters as the reader sees them (a letter with a combining accent), 4002 scalars.
        #expect(throws: EnvelopeError.textTooLong) { try make(text: String(repeating: "e\u{301}", count: 2001)).validate() }
    }

    @Test("a choice is 1 to 20 characters")
    func choiceLength() throws {
        try make(choice: String(repeating: "a", count: 20)).validate()
        #expect(throws: EnvelopeError.malformedChoice) { try make(choice: "").validate() }
        #expect(throws: EnvelopeError.malformedChoice) { try make(choice: String(repeating: "a", count: 21)).validate() }
    }

    @Test("ids, target and time have the node's shape", arguments: [
        ("id", "00112233445566778899AABBCCDDEEFF"), ("id", "0011"), ("id", "00112233445566778899aabbccddeeff0"), ("id", "00112233445566778899aabbccddeefg"), ("org", "Acme"), ("org", "-acme"), ("org", ""),
        ("team", "a_b"), ("account", String(repeating: "a", count: 41)),
    ])
    func malformedIDs(field: String, value: String) {
        let envelope = make(
            id: field == "id" ? value : "00112233445566778899aabbccddeeff",
            org: field == "org" ? value : "acme", team: field == "team" ? value : "app",
            account: field == "account" ? value : "alice"
        )
        #expect(throws: EnvelopeError.malformedID(field: field)) { try envelope.validate() }
    }

    @Test("a target of other characters or the wrong length is refused", arguments: ["", "a b", "a/b", "ア", String(repeating: "a", count: 121)])
    func malformedTarget(value: String) {
        #expect(throws: EnvelopeError.malformedTarget) { try make(target: value).validate() }
    }

    @Test("a target may hold letters, digits and ._#-")
    func target() throws {
        try make(target: "Repo.name_1#12-x").validate()
    }

    @Test("a time that is not RFC 3339 is refused", arguments: ["", "yesterday", "2026-11-24", "2026-11-24 12:00:00"])
    func malformedTime(value: String) {
        #expect(throws: EnvelopeError.malformedTime) { try make(at: value).validate() }
    }

    @Test("an envelope id comes from exactly 16 bytes")
    func makeID() {
        #expect(DecisionEnvelope.makeID(randomBytes: [UInt8](repeating: 0xAB, count: 16)) == String(repeating: "ab", count: 16))
        #expect(DecisionEnvelope.makeID(randomBytes: [UInt8](repeating: 1, count: 15)) == nil)
        #expect(DecisionEnvelope.makeID(randomBytes: [UInt8](repeating: 1, count: 17)) == nil)
        #expect(DecisionEnvelope.makeID(randomBytes: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 255]) == "000102030405060708090a0b0c0d0eff")
    }
}

@Suite("Timestamp")
struct TimestampTests {
    @Test("whole seconds in UTC, ending in Z")
    func format() {
        #expect(Timestamp.string(from: Date(timeIntervalSince1970: 1_795_521_600)) == "2026-11-24T12:00:00Z")
        #expect(Timestamp.string(from: Date(timeIntervalSince1970: 1_795_521_600.9)) == "2026-11-24T12:00:00Z")
    }

    @Test("it reads what it writes, fractions, and offsets")
    func parse() {
        #expect(Timestamp.date(from: "2026-11-24T12:00:00Z") == Date(timeIntervalSince1970: 1_795_521_600))
        #expect(Timestamp.date(from: "2026-11-24T12:00:00.5Z") == Date(timeIntervalSince1970: 1_795_521_600.5))
        #expect(Timestamp.date(from: "2026-11-24T21:00:00+09:00") == Date(timeIntervalSince1970: 1_795_521_600))
    }

    @Test("other text is nil", arguments: ["", "2026-11-24", "12:00:00Z", "2026-11-24T12:00:00", "now"])
    func reject(text: String) {
        #expect(Timestamp.date(from: text) == nil)
    }
}
