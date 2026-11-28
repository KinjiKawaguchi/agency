import AgencyAPI
import AgencyProtocol
import Foundation

/// What removing one of the person's devices comes to, worked out from the node's list (as the web does, agency#442): in which orgs the
/// removal is signed and sent, and why it is not, as a whole or in part.
///
/// A removal takes a device's key out of an org for good: it is added to the org's `revoked` keys and can never sign again, under any id.
/// A device is listed in each org it was added to, and a removal is one change for each of those orgs, signed by this device as its own id
/// there. **The last device of a person in an org cannot be removed from a client**: nobody could sign for the person there any more, and the
/// node does not stop it, so this rule is the client's. If it holds in any org, nothing is removed. An org where this device is not added
/// cannot be signed for: the device stays there, and the person is told.
public struct DeviceRemovalPlan: Equatable, Sendable {
    /// One change to sign and send.
    public struct Target: Equatable, Sendable {
        public let org: String
        /// The id of the device to remove in this org, as the node gave it: it is signed as it is.
        public let deviceID: String
        /// This device's own id in the org, which signs the change.
        public let signer: String
    }

    public let label: String
    public let code: String?
    /// The orgs the removal is signed and sent for, in the node's order.
    public let targets: [Target]
    /// The orgs that list the device and where this device is not added, so that it cannot sign there.
    public let stillIn: [String]
    /// The orgs where it is the person's only device: with any, nothing is removed.
    public let lastIn: [String]
    /// The device to remove is this one: after it, this device cannot sign, and its key cannot be added again.
    public let isThisDevice: Bool

    /// Decisions signed with the key of the device to remove are still kept on this phone, waiting to be sent: removing the device would make
    /// each of them fail for good. A device is this phone's own, or an earlier key of it that was made again.
    public let hasKeptDecisions: Bool

    /// Whether nothing can be removed: the person's last device in some org, or no org that this device can sign for, or a device that signed
    /// decisions that are still kept.
    public var isBlocked: Bool { !lastIn.isEmpty || targets.isEmpty || hasKeptDecisions }

    /// - Parameter hasKeptDecisions: whether decisions signed with the key of the device to remove are kept, waiting to be sent.
    public init(for row: DeviceFacts.Row, in list: DeviceList, ownKey: String, hasKeptDecisions: Bool = false) {
        self.hasKeptDecisions = hasKeptDecisions
        label = row.label
        code = (try? DevicePublicKey(string: row.publicKey))?.fingerprint.pairingCode
        isThisDevice = row.isThisPhone
        let listing = list.orgs.filter { org in org.devices.contains { $0.publicKey == row.publicKey } }
        lastIn = listing.filter { $0.devices.count <= 1 }.map(\.org)
        var targets: [Target] = []
        var stillIn: [String] = []
        for org in listing {
            guard let device = org.devices.first(where: { $0.publicKey == row.publicKey }) else { continue }
            if let signer = DeviceFacts.deviceID(in: list, key: ownKey, org: org.org) {
                targets.append(Target(org: org.org, deviceID: device.id, signer: signer))
            } else {
                stillIn.append(org.org)
            }
        }
        self.targets = lastIn.isEmpty ? targets : []
        self.stillIn = stillIn
    }

    /// The orgs the removal is for, as one line.
    public var orgsText: String { targets.map(\.org).joined(separator: "、") }

    /// What the confirmation says, for a removal that can be made: the question, what it means, where, and for this device what follows.
    public var confirmation: [String] {
        var lines = [Ja.Devices.Remove.body(label, code: code)]
        if targets.count > 1 { lines.append(Ja.Devices.Remove.fromOrgs(orgsText)) }
        if !stillIn.isEmpty { lines.append(Ja.Devices.Remove.stillIn(stillIn.joined(separator: "、"))) }
        if isThisDevice { lines.append(Ja.Devices.Remove.thisDevice) }
        return lines
    }

    /// The question the confirmation asks.
    public var title: String { Ja.Devices.Remove.title(label) }

    /// What the confirmation says, one paragraph each: what removing it means and where, and for a removal that cannot be made why not.
    public var lines: [String] { refusal.map { [$0] } ?? confirmation }

    /// The same as one text.
    public var message: String { lines.joined(separator: "\n") }

    /// Whether the person can confirm: not when the removal is one the client does not make.
    public var canConfirm: Bool { !isBlocked }

    /// Why nothing can be removed, when that is so.
    public var refusal: String? {
        if !lastIn.isEmpty { return Ja.Devices.Remove.last(label, orgs: lastIn.joined(separator: "、")) }
        if hasKeptDecisions { return Ja.Devices.Remove.keptDecisions }
        if targets.isEmpty && !stillIn.isEmpty { return Ja.Devices.Remove.stillIn(stillIn.joined(separator: "、")) }
        return nil
    }
}

/// Removes a device of the person's from the orgs a plan names.
public protocol DeviceRemover: Sendable {
    func remove(_ plan: DeviceRemovalPlan) async throws -> [DeviceChangeResult]
}

extension DeviceChangeSender: DeviceRemover {}
