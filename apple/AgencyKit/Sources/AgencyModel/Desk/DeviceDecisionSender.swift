import AgencyAPI
import AgencyProtocol
import Foundation

/// What the sender needs from the node. `NodeClient` is the real one.
public protocol DecisionPoster: Sendable {
    func me() async throws -> Me
    func devices() async throws -> DeviceList
    func refreshSession() async throws -> Session
    func nodeNow() async -> Date
    func submit(_ decision: SignedDecision, org: String, team: String) async throws -> DecisionResult
}

extension NodeClient: DecisionPoster {}

public enum DeviceDecisionError: Error, Equatable, LocalizedError, Sendable {
    /// This device's key is not among the person's devices in that org.
    case notAdded(org: String)

    public var errorDescription: String? {
        switch self {
        case .notAdded(let org): Ja.Device.notAdded(org: org)
        }
    }
}

/// Signs a prepared decision with this device's key and sends it: the envelope is built from the
/// request's own `answerWith`, stamped with the node's time, checked against the shape the node
/// requires, signed after the person confirms (Face ID, in the app), and posted with the device's id as
/// its signer.
public struct DeviceDecisionSender: DeskSender {
    private let poster: any DecisionPoster
    private let signer: any DeviceSigner
    private let randomBytes: @Sendable () -> [UInt8]

    public init(
        poster: any DecisionPoster, signer: any DeviceSigner,
        randomBytes: @escaping @Sendable () -> [UInt8] = { (0..<16).map { _ in UInt8.random(in: .min ... .max) } }
    ) {
        self.poster = poster
        self.signer = signer
        self.randomBytes = randomBytes
    }

    public func send(_ decision: PreparedDecision) async throws -> DecisionResult {
        _ = try await poster.refreshSession()
        let me = try await poster.me()
        let deviceID = try await deviceID(in: decision.org)
        guard let id = DecisionEnvelope.makeID(randomBytes: randomBytes()) else { throw EnvelopeError.malformedID(field: "id") }
        let now = await poster.nodeNow()
        let envelope = decision.envelope(id: id, account: me.account.id, at: Timestamp.string(from: now))
        try envelope.validate()
        let text = try signedText(kind: .envelope, body: envelope.json)
        let signature = try await signer.sign(text, reason: Ja.Paper.faceIDReason(decision))
        let signed = SignedDecision(
            envelope: envelope, signer: deviceID, signature: signature.base64EncodedString(),
            title: decision.title, choiceLabel: decision.choiceLabel
        )
        return try await poster.submit(signed, org: decision.org, team: decision.team)
    }

    /// The id the node gave this key in `org`, found by comparing keys, never assumed from the fingerprint.
    private func deviceID(in org: String) async throws -> String {
        let key = signer.publicKey.string
        let list = try await poster.devices()
        guard let device = list.orgs.first(where: { $0.org == org })?.devices.first(where: { $0.publicKey == key }) else {
            throw DeviceDecisionError.notAdded(org: org)
        }
        return device.id
    }
}
