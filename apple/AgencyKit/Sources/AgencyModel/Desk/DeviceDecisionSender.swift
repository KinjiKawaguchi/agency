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

/// What signing needs besides the key, kept for a few minutes so that a decision is signed without waiting for the network: the
/// account's id, this device's id in each org, and that the node's clock has been read once (`refreshSession`).
final class SigningFacts: @unchecked Sendable {
    private let lock = NSLock()
    private var account: String?
    private var devices: [String: String] = [:]
    private var readAt: Date?

    /// How long what was read is trusted before it is read again (when the network allows).
    static let lifetime: TimeInterval = 600

    func isFresh(at now: Date) -> Bool { lock.withLock { readAt.map { now.timeIntervalSince($0) < Self.lifetime } ?? false } }
    func knows(org: String) -> Bool { lock.withLock { account != nil && devices[org] != nil } }
    func accountAndDevice(org: String) -> (account: String, device: String)? {
        lock.withLock { account.flatMap { account in devices[org].map { (account, $0) } } }
    }
    func keep(account: String, devices: [String: String], at now: Date) {
        lock.withLock {
            self.account = account
            self.devices.merge(devices) { _, new in new }
            readAt = now
        }
    }
}

/// Signs a prepared decision with this device's key: the envelope is built from the request's own `answerWith`, stamped with the node's
/// time, checked against the shape the node requires, and signed after the person confirms (Face ID, in the app), with the device's id
/// as its signer. It sends nothing: the signed decision is kept in the outbox and sent from there. What it needs from the node (the
/// account, this device's id, the node's clock) is read once and kept for ten minutes, so that deciding does not wait for the network;
/// when the network is down and something was read before, that is used.
public struct DeviceDecisionSigner: DeskSigner {
    private let poster: any DecisionPoster
    private let signer: any DeviceSigner
    private let randomBytes: @Sendable () -> [UInt8]
    private let sent: SentDecisions?
    private let facts = SigningFacts()
    private let clock: @Sendable () -> Date

    /// - Parameter sent: remembers the id of each envelope this signs, so that the news does not announce its success.
    public init(
        poster: any DecisionPoster, signer: any DeviceSigner, sent: SentDecisions? = nil,
        randomBytes: @escaping @Sendable () -> [UInt8] = { (0..<16).map { _ in UInt8.random(in: .min ... .max) } },
        clock: @escaping @Sendable () -> Date = { Date() }
    ) {
        self.poster = poster
        self.signer = signer
        self.sent = sent
        self.randomBytes = randomBytes
        self.clock = clock
    }

    public func sign(_ decision: PreparedDecision) async throws -> SignedDecision {
        let (account, deviceID) = try await accountAndDevice(in: decision.org)
        guard let id = DecisionEnvelope.makeID(randomBytes: randomBytes()) else { throw EnvelopeError.malformedID(field: "id") }
        let now = await poster.nodeNow()
        let envelope = decision.envelope(id: id, account: account, at: Timestamp.string(from: now))
        try envelope.validate()
        sent?.record(id)
        let text = try signedText(kind: .envelope, body: envelope.json)
        let signature = try await signer.sign(text, reason: Ja.Paper.faceIDReason(decision))
        return SignedDecision(
            envelope: envelope, signer: deviceID, signature: signature.base64EncodedString(),
            title: decision.title, choiceLabel: decision.choiceLabel
        )
    }

    /// The account and this device's id in `org`: from what was read lately, else read now; when reading fails and something was read
    /// before, that is used.
    private func accountAndDevice(in org: String) async throws -> (account: String, device: String) {
        let now = clock()
        if facts.isFresh(at: now), let known = facts.accountAndDevice(org: org) { return known }
        do {
            _ = try await poster.refreshSession()
            let me = try await poster.me()
            let key = signer.publicKey.string
            let list = try await poster.devices()
            var devices: [String: String] = [:]
            for entry in list.orgs {
                if let device = entry.devices.first(where: { $0.publicKey == key }) { devices[entry.org] = device.id }
            }
            facts.keep(account: me.account.id, devices: devices, at: now)
        } catch {
            // The network is down: what was read before is better than no decision.
            if let known = facts.accountAndDevice(org: org) { return known }
            throw error
        }
        guard let known = facts.accountAndDevice(org: org) else { throw DeviceDecisionError.notAdded(org: org) }
        return known
    }
}
