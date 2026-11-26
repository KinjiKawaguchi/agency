@testable import AgencyAPI
import AgencyProtocol
import Foundation
import Testing

/// Every word of the node's that an app switches on reads when it is new (`docs/compatibility.md`: readers ignore what they do
/// not know): the whole answer still reads, and the new word is never one of the known ones.
@Suite("Words of the node that the app does not know")
struct OpenWireEnumTests {
    private func read<T: Decodable>(_ type: T.Type, _ json: String) throws -> T { try JSONDecoder().decode(T.self, from: Data(json.utf8)) }

    @Test("a new word in each enum of plain words is that enum's unknown, and a known word is still itself")
    func plainWords() throws {
        func word<T: OpenWireEnum & Equatable>(_ type: T.Type, known: String, is expected: T) throws {
            #expect(try read(T.self, #""\#(known)""#) == expected, Comment(rawValue: "\(T.self) \(known)"))
            #expect(try read(T.self, #""aWordFromTheFuture""#) == T.unknown, Comment(rawValue: "\(T.self)"))
            #expect(expected != T.unknown)
        }
        try word(TeamRef.Location.self, known: "remote", is: .remote)
        try word(Funding.Kind.self, known: "apiKey", is: .apiKey)
        try word(UsageWindow.Name.self, known: "fiveHour", is: .fiveHour)
        try word(OrgMember.Role.self, known: "answerer", is: .answerer)
        try word(RegisteredPushToken.Environment.self, known: "sandbox", is: .sandbox)
        try word(PersonStatus.Kind.self, known: "human", is: .human)
        try word(PersonStatus.Role.self, known: "lead", is: .lead)
        try word(PersonStatus.State.self, known: "away", is: .away)
        try word(Request.Kind.self, known: "proposal", is: .proposal)
        try word(WorkItem.Status.self, known: "inProgress", is: .inProgress)
        try word(BlockedReason.self, known: "external", is: .external)
        try word(DecisionResult.Outcome.self, known: "queued", is: .queued)
        try word(DecisionEnvelope.Kind.self, known: "judge", is: .judge)
    }

    @Test("a decision that went somewhere the app does not know is read as unknown, and the others of the log still read")
    func decisionState() throws {
        let entry = #"{"id":"e1","org":"acme","team":"app","kind":"answer","target":"r-1","text":"","decidedAt":"2026-11-26T09:00:00Z","state":\#("STATE")}"#
        func log(_ state: String) throws -> DecisionLog {
            try read(DecisionLog.self, #"{"entries":[\#(entry.replacingOccurrences(of: "STATE", with: state)),\#(entry.replacingOccurrences(of: "STATE", with: #"{"kind":"applied"}"#))]}"#)
        }
        let future = try log(#"{"kind":"paused","since":"2026-11-26T09:00:00Z"}"#)
        #expect(future.entries.map(\.state) == [.unknown("paused"), .applied])
        #expect(try log(#"{"kind":"refused","message":"no"}"#).entries[0].state == .refused(message: "no"))
    }

    @Test("someone a runtime says a person waits on, in a way the app does not know, is read, and the people around still read")
    func waitingOn() throws {
        #expect(try read(WaitingOn.self, #"{"kind":"team","team":"ops"}"#) == .unknown("team"))
        #expect(try read(WaitingOn.self, #"{"kind":"lead"}"#) == .lead)
        #expect(try read(WaitingOn.self, #"{"kind":"person","person":"bob"}"#) == .person("bob"))
    }

    @Test("a source of a team's view the app does not know is read, and the view with it")
    func source() throws {
        #expect(try read(TeamSnapshot.Source.self, #"{"kind":"mirror","since":"x"}"#) == .unknown("mirror"))
        #expect(try read(TeamSnapshot.Source.self, #"{"kind":"local"}"#) == .local)
    }

    @Test("a request of a kind the app does not know is read whole, with the way to answer it")
    func request() throws {
        let json = #"{"id":"r-9","kind":"vote","title":"t","context":"c","from":"backend","createdAt":"2026-11-24T11:00:00Z","blocks":[],"options":[],"answerWith":{"kind":"tally","target":"r-9"}}"#
        let request = try read(Request.self, json)
        #expect(request.kind == .unknown && request.answerWith.kind == .unknown && request.answerWith.target == "r-9")
    }

    @Test("an envelope of a kind the app does not know is never made: it is refused before anything is signed")
    func neverSigned() throws {
        let envelope = DecisionEnvelope(
            id: String(repeating: "a", count: 32), kind: .unknown, org: "acme", team: "app", target: "r-1", text: "yes", choice: nil, account: "alice", at: "2026-11-26T09:00:00Z"
        )
        #expect(throws: EnvelopeError.unknownKind) { try envelope.validate() }
    }
}
