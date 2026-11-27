import AgencyAPI
import AgencyProtocol
import Foundation

/// What adding a device needs from the node. `NodeClient` is the real one.
public protocol DeviceChangePoster: Sendable {
    func me() async throws -> Me
    func devices() async throws -> DeviceList
    func refreshSession() async throws -> Session
    func nodeNow() async -> Date
    func submitDeviceChange(_ change: SignedDeviceChange) async throws -> DeviceChangeResult
}

extension NodeClient: DeviceChangePoster {}

public enum DeviceChangeSenderError: Error, Equatable, LocalizedError, Sendable {
    /// The request's key is not a key, or its id or code is not the one its key makes.
    case malformedRequest
    /// The code the node showed is not the code the key makes: the person would compare the wrong one.
    case codeMismatch
    /// No org holds both this device and lacks the new one.
    case nowhereToAdd
    /// The removal is one the client does not make: the person's last device in an org, or no org this device can sign for.
    case nowhereToRemove

    public var errorDescription: String? {
        switch self {
        case .malformedRequest: Ja.Devices.malformedRequest
        case .codeMismatch: Ja.Devices.codeMismatch
        case .nowhereToAdd: Ja.Devices.nowhereToAdd
        case .nowhereToRemove: Ja.Devices.Remove.cannotHere
        }
    }
}

/// A later org's change failed after the node had answered the changes before it: what it answered is not lost with the failure.
public struct DeviceChangePartlyAnswered: Error, Sendable {
    public let answered: [DeviceChangeResult]
    public let cause: any Error
}

/// Adds another device to the person's orgs: for each org that lacks it and that holds this device, a
/// change is built, stamped with the node's time, signed after Face ID, and posted. This device signs
/// as the id the node gave it in that org.
public struct DeviceChangeSender: Sendable {
    private let poster: any DeviceChangePoster
    private let signer: any DeviceSigner

    public init(poster: any DeviceChangePoster, signer: any DeviceSigner) {
        self.poster = poster
        self.signer = signer
    }

    /// - Returns: what the node answered for each org, in the order they were added.
    /// - Throws: `DeviceChangePartlyAnswered` when a change fails after an earlier one was answered; the failure itself otherwise.
    public func add(_ request: DeviceRequest) async throws -> [DeviceChangeResult] {
        let key = try verified(request)
        _ = try await poster.refreshSession()
        let me = try await poster.me()
        let list = try await poster.devices()
        let own = signer.publicKey.string
        let targets = DeviceFacts.orgsLacking(list, key: key.string).compactMap { org in
            DeviceFacts.deviceID(in: list, key: own, org: org).map { (org: org, signer: $0) }
        }
        guard !targets.isEmpty else { throw DeviceChangeSenderError.nowhereToAdd }
        var results: [DeviceChangeResult] = []
        for target in targets {
            let now = Timestamp.string(from: await poster.nodeNow())
            let change = DeviceChange(
                org: target.org, account: me.account.id,
                operation: .add(.init(id: request.deviceId, label: request.rawLabel, publicKey: key.string, addedAt: now)), at: now
            )
            try change.validate()
            let text = try signedText(kind: .device, body: change.json)
            let signature = try await signer.sign(text, reason: Ja.Devices.faceIDReason(request.label))
            let signed = SignedDeviceChange(change: change, signer: target.signer, signature: signature.base64EncodedString())
            do {
                results.append(try await poster.submitDeviceChange(signed))
            } catch where !results.isEmpty {
                throw DeviceChangePartlyAnswered(answered: results, cause: error)
            }
        }
        return results
    }

    /// Removes a device of the person's from each org the plan names: a change for each, stamped with the node's time, signed after Face ID by
    /// this device as its own id in that org, and posted. What is signed is the id of the device as the node gave it, never a shown one.
    /// - Returns: what the node answered for each org, in the order they were sent.
    /// - Throws: `DeviceChangePartlyAnswered` when a change fails after an earlier one was answered; the failure itself otherwise.
    public func remove(_ plan: DeviceRemovalPlan) async throws -> [DeviceChangeResult] {
        guard !plan.isBlocked else { throw DeviceChangeSenderError.nowhereToRemove }
        _ = try await poster.refreshSession()
        let me = try await poster.me()
        var changes: [DeviceChange] = []
        for target in plan.targets {
            let now = Timestamp.string(from: await poster.nodeNow())
            let change = DeviceChange(org: target.org, account: me.account.id, operation: .remove(target.deviceID), at: now)
            try change.validate()
            changes.append(change)
        }
        // All of them are signed on one confirmation, before the first is sent: the person is asked once for what the confirmation listed.
        let texts = try changes.map { try signedText(kind: .device, body: $0.json) }
        let signatures = try await signer.signEach(texts, reason: Ja.Devices.Remove.faceIDReason(plan.label))
        var results: [DeviceChangeResult] = []
        for (index, target) in plan.targets.enumerated() {
            let signed = SignedDeviceChange(change: changes[index], signer: target.signer, signature: signatures[index].base64EncodedString())
            do {
                results.append(try await poster.submitDeviceChange(signed))
            } catch where !results.isEmpty {
                throw DeviceChangePartlyAnswered(answered: results, cause: error)
            }
        }
        return results
    }

    /// The request's key, once the node's id and code for it are checked against the key itself. A node
    /// that showed a different code would have the person compare the wrong one and add a different key.
    private func verified(_ request: DeviceRequest) throws -> DevicePublicKey {
        guard let key = try? DevicePublicKey(string: request.publicKey), request.deviceId == key.fingerprint.deviceID else {
            throw DeviceChangeSenderError.malformedRequest
        }
        guard request.pairingCode == key.fingerprint.pairingCode else { throw DeviceChangeSenderError.codeMismatch }
        return key
    }
}
