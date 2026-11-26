import Foundation

public enum DeviceChangeError: Error, Equatable, Sendable {
    case malformedID(field: String)
    case malformedLabel
    case malformedKey
    case malformedTime
}

/// A change to one person's devices in one org: adding a device, or removing one. A device of that
/// person signs it (`signedText(kind: .device, ...)`); the home node relays it to the org's root node.
public struct DeviceChange: Equatable, Sendable {
    public struct NewDevice: Equatable, Sendable {
        public let id: String
        public let label: String
        /// The key as the protocol writes it: bare base64 for Ed25519, `p256:` and base64 for P-256.
        public let publicKey: String
        public let addedAt: String

        public init(id: String, label: String, publicKey: String, addedAt: String) {
            self.id = id
            self.label = label
            self.publicKey = publicKey
            self.addedAt = addedAt
        }
    }

    public enum Operation: Equatable, Sendable {
        case add(NewDevice)
        /// The id of the device to remove.
        case remove(String)
    }

    public static let maxLabelLength = 80

    public let org: String
    public let account: String
    public let operation: Operation
    public let at: String

    public init(org: String, account: String, operation: Operation, at: String) {
        self.org = org
        self.account = account
        self.operation = operation
        self.at = at
    }

    /// The body that is signed and sent: `{ org, account, op, device, at }`, where `device` is the whole
    /// device for an add and only its id for a removal.
    public var json: JSONValue {
        var members: [JSONValue.Member] = [.init("org", .string(org)), .init("account", .string(account)), .init("at", .string(at))]
        switch operation {
        case .add(let device):
            members.append(.init("op", "add"))
            members.append(.init("device", .object([
                .init("id", .string(device.id)), .init("label", .string(device.label)),
                .init("publicKey", .string(device.publicKey)), .init("addedAt", .string(device.addedAt)),
            ])))
        case .remove(let id):
            members.append(.init("op", "remove"))
            members.append(.init("device", .string(id)))
        }
        return .object(members)
    }

    /// The shape the node requires, checked before anyone is asked for Face ID.
    public func validate() throws {
        for (field, value) in [("org", org), ("account", account)] where !DecisionEnvelope.isID(value) {
            throw DeviceChangeError.malformedID(field: field)
        }
        guard Timestamp.date(from: at) != nil else { throw DeviceChangeError.malformedTime }
        switch operation {
        case .add(let device):
            guard DecisionEnvelope.isID(device.id) else { throw DeviceChangeError.malformedID(field: "device") }
            let label = device.label.unicodeScalars.count
            guard (1...Self.maxLabelLength).contains(label), device.label == device.label.trimmingCharacters(in: .whitespacesAndNewlines) else {
                throw DeviceChangeError.malformedLabel
            }
            guard (try? DevicePublicKey(string: device.publicKey)) != nil else { throw DeviceChangeError.malformedKey }
            guard Timestamp.date(from: device.addedAt) != nil else { throw DeviceChangeError.malformedTime }
        case .remove(let id):
            guard DecisionEnvelope.isID(id) else { throw DeviceChangeError.malformedID(field: "device") }
        }
    }
}
