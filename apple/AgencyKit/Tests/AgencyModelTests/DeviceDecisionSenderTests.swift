@testable import AgencyAPI
import AgencyProtocol
@testable import AgencyModel
import Foundation
import Testing

private final class FakePoster: DecisionPoster, @unchecked Sendable {
    private let lock = NSLock()
    var devices: DeviceList
    var me: Me = Fixture.decode(Me.self, "me")
    var now = Date(timeIntervalSince1970: 1_795_521_600)
    var submitResult: Result<DecisionResult, Error> = .success(DecisionResult(outcome: .applied, message: nil))
    private var _calls: [String] = []
    private var _submitted: [(SignedDecision, String, String)] = []

    init(devices: DeviceList) { self.devices = devices }

    var calls: [String] { lock.withLock { _calls } }
    var submitted: [(decision: SignedDecision, org: String, team: String)] { lock.withLock { _submitted.map { ($0.0, $0.1, $0.2) } } }

    func me() async throws -> Me { lock.withLock { _calls.append("me") }; return me }
    func devices() async throws -> DeviceList { lock.withLock { _calls.append("devices") }; return devices }
    func refreshSession() async throws -> Session {
        lock.withLock { _calls.append("session") }
        return try JSONDecoder().decode(Session.self, from: Data(#"{"writeToken":"w","now":"2026-11-24T12:00:00Z"}"#.utf8))
    }
    func nodeNow() async -> Date { lock.withLock { _calls.append("now") }; return now }
    func submit(_ decision: SignedDecision, org: String, team: String) async throws -> DecisionResult {
        lock.withLock { _calls.append("submit"); _submitted.append((decision, org, team)) }
        return try submitResult.get()
    }
}

private final class CountingSigner: DeviceSigner, @unchecked Sendable {
    let inner: SoftwareP256Signer
    private let lock = NSLock()
    private var _reasons: [String] = []
    var failure: Error?

    init() throws { inner = try SoftwareP256Signer(rawPrivateKey: Vectors.shared.p256.privateKey) }
    var publicKey: DevicePublicKey { inner.publicKey }
    var reasons: [String] { lock.withLock { _reasons } }

    func sign(_ text: Data, reason: String) async throws -> Data {
        lock.withLock { _reasons.append(reason) }
        if let failure { throw failure }
        return try await inner.sign(text, reason: reason)
    }
}

/// The Vectors file the protocol tests read is also the source of this device key.
private enum Vectors {
    struct P256 { let privateKey: Data }
    struct Shared { let p256: P256 }
    static let shared = Shared(p256: P256(privateKey: Data(repeating: 0x0b, count: 32)))
}

@Suite("DeviceDecisionSender")
struct DeviceDecisionSenderTests {
    private let signer: CountingSigner
    private let deviceKey: String
    private let team = Teams.team()

    init() throws {
        signer = try CountingSigner()
        deviceKey = signer.publicKey.string
    }

    private func list(org: String = "acme", id: String = "d-a652124afda7", key: String? = nil) throws -> DeviceList {
        let json = """
        {"orgs":[{"org":"\(org)","devices":[{"addedAt":"2026-11-24T12:00:00Z","id":"\(id)","label":"iPhone","publicKey":"\(key ?? deviceKey)"}]}],"requests":[]}
        """
        return try JSONDecoder().decode(DeviceList.self, from: Data(json.utf8))
    }

    private func sender(_ poster: FakePoster) -> DeviceDecisionSender {
        DeviceDecisionSender(poster: poster, signer: signer, randomBytes: { (0..<16).map(UInt8.init) })
    }

    private func decision(_ id: String, _ reply: DeskReply) throws -> PreparedDecision {
        try #require(Desk.openItems([team]).first { $0.request.id == id }).prepare(reply)
    }

    @Test("a decision is built, signed by this key and posted with the device's id as its signer")
    func sends() async throws {
        let poster = FakePoster(devices: try list())
        let prepared = try decision(Teams.questionID, .choose(optionKey: "b", note: "理由あり"))
        let result = try await sender(poster).send(prepared)
        #expect(result.outcome == .applied)
        let sent = try #require(poster.submitted.first)
        #expect(sent.org == "acme" && sent.team == "app")
        #expect(sent.decision.signer == "d-a652124afda7")
        let envelope = sent.decision.envelope
        #expect(envelope.id == "000102030405060708090a0b0c0d0e0f")
        #expect(envelope.account == "alice")
        #expect(envelope.at == "2026-11-24T12:00:00Z")
        #expect(envelope.target == Teams.questionID && envelope.kind == .answer)
        #expect(envelope.choice == "b" && envelope.text == "理由あり")
        #expect(sent.decision.title == "Which database should the app use in production?")
        #expect(sent.decision.choiceLabel == prepared.choiceLabel)
    }

    @Test("the signature verifies with this key over exactly the text the node checks")
    func signature() async throws {
        let poster = FakePoster(devices: try list())
        _ = try await sender(poster).send(decision(Teams.proposalID, .comment("もう少し待ってください 🙏")))
        let sent = try #require(poster.submitted.first).decision
        let text = try signedText(kind: .envelope, body: sent.envelope.json)
        let signature = try #require(Data(base64Encoded: sent.signature))
        #expect(signature.count == 64)
        #expect(signer.publicKey.verifies(signature, over: text))
        #expect(!signer.publicKey.verifies(signature, over: Data("agency/v1/device\n".utf8) + text.dropFirst("agency/v1/envelope\n".utf8.count)))
    }

    @Test("a comment is signed without a choice")
    func comment() async throws {
        let poster = FakePoster(devices: try list())
        _ = try await sender(poster).send(decision(Teams.proposalID, .comment("待って")))
        let sent = try #require(poster.submitted.first).decision
        #expect(sent.envelope.choice == nil)
        #expect(sent.choiceLabel == nil)
    }

    @Test("a decision the board waits on is signed as a judgement of its item")
    func judgement() async throws {
        let poster = FakePoster(devices: try list())
        _ = try await sender(poster).send(decision(Teams.decisionID, .answer("終了します")))
        let envelope = try #require(poster.submitted.first).decision.envelope
        #expect(envelope.kind == .judge && envelope.target == "app#14")
    }

    @Test("the session and the node's time come first, so the time stamped is the node's")
    func order() async throws {
        let poster = FakePoster(devices: try list())
        poster.now = Date(timeIntervalSince1970: 1_795_521_600 + 7 * 60)
        _ = try await sender(poster).send(decision(Teams.questionID, .choose(optionKey: "a", note: "")))
        #expect(poster.calls.first == "session")
        #expect(poster.calls.firstIndex(of: "now")! < poster.calls.firstIndex(of: "submit")!)
        #expect(try #require(poster.submitted.first).decision.envelope.at == "2026-11-24T12:07:00Z")
    }

    @Test("Face ID is asked for with the action and the title")
    func reason() async throws {
        let poster = FakePoster(devices: try list())
        _ = try await sender(poster).send(decision(Teams.proposalID, .choose(optionKey: "approve", note: "")))
        _ = try await DeviceDecisionSender(poster: FakePoster(devices: try list()), signer: signer).send(decision(Teams.proposalID, .comment("x")))
        #expect(signer.reasons == ["決裁する: Add a second frontend agent", "コメントを送る: Add a second frontend agent"])
    }

    @Test("a key that is not among the devices of the org is refused before Face ID")
    func notAdded() async throws {
        for devices in [try list(key: "p256:BAAAAA=="), try list(org: "other")] {
            let poster = FakePoster(devices: devices)
            await #expect(throws: DeviceDecisionError.notAdded(org: "acme")) {
                try await sender(poster).send(decision(Teams.questionID, .choose(optionKey: "a", note: "")))
            }
            #expect(poster.submitted.isEmpty)
        }
        #expect(signer.reasons.isEmpty)
    }

    @Test("the not-added message is in Japanese and names the org")
    func notAddedMessage() {
        #expect(DeviceDecisionError.notAdded(org: "acme").errorDescription == "この端末は acme に追加されていません。先に端末を追加してください。")
    }

    @Test("a text the node would refuse is refused before Face ID")
    func invalidEnvelope() async throws {
        let poster = FakePoster(devices: try list())
        let long = String(repeating: "あ", count: 4001)
        await #expect(throws: EnvelopeError.textTooLong) {
            try await sender(poster).send(decision(Teams.proposalID, .comment(long)))
        }
        #expect(signer.reasons.isEmpty)
        #expect(poster.submitted.isEmpty)
    }

    @Test("when the person cancels Face ID, nothing is posted")
    func canceled() async throws {
        struct Canceled: Error, Equatable {}
        signer.failure = Canceled()
        let poster = FakePoster(devices: try list())
        await #expect(throws: Canceled()) {
            try await sender(poster).send(decision(Teams.questionID, .choose(optionKey: "a", note: "")))
        }
        #expect(poster.submitted.isEmpty)
    }

    @Test("the node's refusal is passed on")
    func refused() async throws {
        let poster = FakePoster(devices: try list())
        poster.submitResult = .failure(NodeError.http(status: 409, message: "refused"))
        await #expect(throws: NodeError.http(status: 409, message: "refused")) {
            try await sender(poster).send(decision(Teams.questionID, .choose(optionKey: "a", note: "")))
        }
    }

    @Test("the device id is the node's, found by key, not worked out from the fingerprint")
    func idFromTheList() async throws {
        let poster = FakePoster(devices: try list(id: "d-chosen-by-node"))
        _ = try await sender(poster).send(decision(Teams.questionID, .choose(optionKey: "a", note: "")))
        #expect(try #require(poster.submitted.first).decision.signer == "d-chosen-by-node")
    }
}
