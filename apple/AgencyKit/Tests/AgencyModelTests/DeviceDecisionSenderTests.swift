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
    var submitResult: Result<DecisionResult, Error> = .success(try! JSONDecoder().decode(DecisionResult.self, from: Data(#"{"outcome":"applied"}"#.utf8)))
    /// What the node's reads answer with: set to fail them, as when the network is down.
    var readFailure: Error?
    private var _calls: [String] = []
    private var _submitted: [(SignedDecision, String, String)] = []

    init(devices: DeviceList) { self.devices = devices }

    var calls: [String] { lock.withLock { _calls } }
    var submitted: [(decision: SignedDecision, org: String, team: String)] { lock.withLock { _submitted.map { ($0.0, $0.1, $0.2) } } }

    func me() async throws -> Me { lock.withLock { _calls.append("me") }; if let readFailure { throw readFailure }; return me }
    func devices() async throws -> DeviceList { lock.withLock { _calls.append("devices") }; if let readFailure { throw readFailure }; return devices }
    func refreshSession() async throws -> Session {
        lock.withLock { _calls.append("session") }
        if let readFailure { throw readFailure }
        return try JSONDecoder().decode(Session.self, from: Data(#"{"writeToken":"w","now":"2026-11-24T12:00:00Z"}"#.utf8))
    }
    func nodeNow() async -> Date { lock.withLock { _calls.append("now") }; return now }
    func submit(_ decision: SignedDecision, org: String, team: String) async throws -> DecisionResult {
        lock.withLock { _calls.append("submit"); _submitted.append((decision, org, team)) }
        return try submitResult.get()
    }
}

private final class Clock: @unchecked Sendable {
    private let lock = NSLock()
    private var current: Date
    init(_ start: Date) { current = start }
    var now: Date { lock.withLock { current } }
    func advance(_ seconds: TimeInterval) { lock.withLock { current = current.addingTimeInterval(seconds) } }
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

@Suite("DeviceDecisionSigner")
struct DeviceDecisionSignerTests {
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

    private func sender(_ poster: FakePoster, clock: @escaping @Sendable () -> Date = { Date() }) -> DeviceDecisionSigner {
        DeviceDecisionSigner(poster: poster, signer: signer, randomBytes: { (0..<16).map(UInt8.init) }, clock: clock)
    }

    private func decision(_ id: String, _ reply: DeskReply) throws -> PreparedDecision {
        try #require(Desk.openItems([team]).first { $0.request.id == id }).prepare(reply)
    }

    @Test("a decision is built and signed by this key with the device's id as its signer, and nothing is posted")
    func signs() async throws {
        let poster = FakePoster(devices: try list())
        let prepared = try decision(Teams.questionID, .choose(optionKey: "b", note: "理由あり"))
        let signed = try await sender(poster).sign(prepared)
        #expect(poster.submitted.isEmpty, "sending is the outbox's")
        #expect(signed.signer == "d-a652124afda7")
        let envelope = signed.envelope
        #expect(envelope.id == "000102030405060708090a0b0c0d0e0f")
        #expect(envelope.account == "alice")
        #expect(envelope.at == "2026-11-24T12:00:00Z")
        #expect(envelope.target == Teams.questionID && envelope.kind == .answer)
        #expect(envelope.choice == "b" && envelope.text == "理由あり")
        #expect(signed.title == "Which database should the app use in production?")
        #expect(signed.choiceLabel == prepared.choiceLabel)
    }

    @Test("the signature verifies with this key over exactly the text the node checks")
    func signature() async throws {
        let poster = FakePoster(devices: try list())
        let sent = try await sender(poster).sign(decision(Teams.proposalID, .comment("もう少し待ってください 🙏")))
        let text = try signedText(kind: .envelope, body: sent.envelope.json)
        let signature = try #require(Data(base64Encoded: sent.signature))
        #expect(signature.count == 64)
        #expect(signer.publicKey.verifies(signature, over: text))
        #expect(!signer.publicKey.verifies(signature, over: Data("agency/v1/device\n".utf8) + text.dropFirst("agency/v1/envelope\n".utf8.count)))
    }

    @Test("a comment is signed without a choice")
    func comment() async throws {
        let poster = FakePoster(devices: try list())
        let sent = try await sender(poster).sign(decision(Teams.proposalID, .comment("待って")))
        #expect(sent.envelope.choice == nil)
        #expect(sent.choiceLabel == nil)
    }

    @Test("a decision the board waits on is signed as a judgement of its item")
    func judgement() async throws {
        let poster = FakePoster(devices: try list())
        let envelope = try await sender(poster).sign(decision(Teams.decisionID, .answer("終了します"))).envelope
        #expect(envelope.kind == .judge && envelope.target == "app#14")
    }

    @Test("the session and the node's time come first, so the time stamped is the node's")
    func order() async throws {
        let poster = FakePoster(devices: try list())
        poster.now = Date(timeIntervalSince1970: 1_795_521_600 + 7 * 60)
        let signed = try await sender(poster).sign(decision(Teams.questionID, .choose(optionKey: "a", note: "")))
        #expect(poster.calls.first == "session")
        #expect(poster.calls.contains("now"))
        #expect(signed.envelope.at == "2026-11-24T12:07:00Z")
    }

    @Test("what the node said is read once and kept: the second decision makes no call to the node but for its time")
    func keepsWhatItRead() async throws {
        let poster = FakePoster(devices: try list())
        let signer = sender(poster)
        _ = try await signer.sign(decision(Teams.questionID, .choose(optionKey: "a", note: "")))
        let afterFirst = poster.calls.count
        _ = try await signer.sign(decision(Teams.proposalID, .comment("x")))
        #expect(Array(poster.calls.dropFirst(afterFirst)) == ["now"], "the network is not needed to decide")
    }

    @Test("after ten minutes it is read again, and when the network is down then, what was read before is used")
    func staleThenOffline() async throws {
        let poster = FakePoster(devices: try list())
        let start = Date(timeIntervalSince1970: 1_800_000_000)
        let clock = Clock(start)
        let signer = sender(poster, clock: { clock.now })
        _ = try await signer.sign(decision(Teams.questionID, .choose(optionKey: "a", note: "")))
        clock.advance(601)
        let before = poster.calls.filter { $0 == "session" }.count
        _ = try await signer.sign(decision(Teams.proposalID, .comment("x")))
        #expect(poster.calls.filter { $0 == "session" }.count == before + 1, "read again")
        clock.advance(601)
        poster.readFailure = URLError(.notConnectedToInternet)
        let offline = try await signer.sign(decision(Teams.proposalID, .comment("y")))
        #expect(offline.signer == "d-a652124afda7", "decided without the network, with what was read")
    }

    @Test("with nothing read yet and the network down, there is no decision to sign")
    func offlineAtFirst() async throws {
        let poster = FakePoster(devices: try list())
        poster.readFailure = URLError(.notConnectedToInternet)
        await #expect(throws: URLError.self) {
            try await sender(poster).sign(decision(Teams.questionID, .choose(optionKey: "a", note: "")))
        }
        #expect(signer.reasons.isEmpty)
    }

    @Test("Face ID is asked for with the action and the title")
    func reason() async throws {
        let poster = FakePoster(devices: try list())
        _ = try await sender(poster).sign(decision(Teams.proposalID, .choose(optionKey: "approve", note: "")))
        _ = try await DeviceDecisionSigner(poster: FakePoster(devices: try list()), signer: signer).sign(decision(Teams.proposalID, .comment("x")))
        #expect(signer.reasons == ["決裁する: Add a second frontend agent", "コメントを送る: Add a second frontend agent"])
    }

    @Test("a key that is not among the devices of the org is refused before Face ID")
    func notAdded() async throws {
        for devices in [try list(key: "p256:BAAAAA=="), try list(org: "other")] {
            let poster = FakePoster(devices: devices)
            await #expect(throws: DeviceDecisionError.notAdded(org: "acme")) {
                try await sender(poster).sign(decision(Teams.questionID, .choose(optionKey: "a", note: "")))
            }
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
            try await sender(poster).sign(decision(Teams.proposalID, .comment(long)))
        }
        #expect(signer.reasons.isEmpty)
    }

    @Test("when the person cancels Face ID, nothing is signed")
    func canceled() async throws {
        struct Canceled: Error, Equatable {}
        signer.failure = Canceled()
        let poster = FakePoster(devices: try list())
        await #expect(throws: Canceled()) {
            try await sender(poster).sign(decision(Teams.questionID, .choose(optionKey: "a", note: "")))
        }
    }

    @Test("the device id is the node's, found by key, not worked out from the fingerprint")
    func idFromTheList() async throws {
        let poster = FakePoster(devices: try list(id: "d-chosen-by-node"))
        let signed = try await sender(poster).sign(decision(Teams.questionID, .choose(optionKey: "a", note: "")))
        #expect(signed.signer == "d-chosen-by-node")
    }
}
