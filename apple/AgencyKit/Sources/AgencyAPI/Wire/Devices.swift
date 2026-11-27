import AgencyProtocol
import Foundation

/// `GET /api/devices`: the account's devices in each of its orgs, and the devices waiting to be added.
public struct DeviceList: Decodable, Equatable, Sendable {
    public let orgs: [OrgDevices]
    public let requests: [DeviceRequest]
    /// What the node says about itself, only to its owner, so that a device waiting to be added can be told
    /// the exact command for this node's machine. Absent from every other answer.
    public let node: NodeHints?
}

/// A node's description of itself, for the pairing instructions.
public struct NodeHints: Decodable, Equatable, Sendable {
    public let url: String?
    public let dataDir: String?
    /// The command to approve a first device on the node's machine, written for a person to read and to
    /// copy: the path of the running program, `--dir` only when it differs from the default.
    public let command: String?
}

public struct OrgDevices: Decodable, Equatable, Sendable {
    public let org: String
    public let devices: [Device]
}

/// A device that is added to an org: its key may sign that person's decisions there.
public struct Device: Decodable, Equatable, Sendable {
    /// `d-` and the first 12 hex characters of the key's fingerprint.
    public let id: String
    @Shown public private(set) var label: String
    /// The key as the protocol writes it: bare base64 for Ed25519, `p256:` and base64 for P-256.
    public let publicKey: String
    public let addedAt: Instant
}

/// A device that asked to be added and waits for one of the person's devices (or the node's terminal) to add it.
public struct DeviceRequest: Decodable, Equatable, Sendable {
    public let deviceId: String
    @Shown public private(set) var label: String
    /// `dbc2-9825-1c51`: what the person compares on both devices.
    public let pairingCode: String
    public let publicKey: String
    public let requestedAt: Instant

    /// The label as the node wrote it, for what is signed and sent back (the change that adds the device): never the shown one.
    public var rawLabel: String { $label }
}
