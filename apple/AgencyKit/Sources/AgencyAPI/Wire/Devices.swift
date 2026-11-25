import Foundation

/// `GET /api/devices`: the account's devices in each of its orgs, and the devices waiting to be added.
public struct DeviceList: Decodable, Equatable, Sendable {
    public let orgs: [OrgDevices]
    public let requests: [DeviceRequest]
}

public struct OrgDevices: Decodable, Equatable, Sendable {
    public let org: String
    public let devices: [Device]
}

/// A device that is added to an org: its key may sign that person's decisions there.
public struct Device: Decodable, Equatable, Sendable {
    /// `d-` and the first 12 hex characters of the key's fingerprint.
    public let id: String
    public let label: String
    /// The key as the protocol writes it: bare base64 for Ed25519, `p256:` and base64 for P-256.
    public let publicKey: String
    public let addedAt: Instant
}

/// A device that asked to be added and waits for one of the person's devices (or the node's terminal) to add it.
public struct DeviceRequest: Decodable, Equatable, Sendable {
    public let deviceId: String
    public let label: String
    /// `dbc2-9825-1c51`: what the person compares on both devices.
    public let pairingCode: String
    public let publicKey: String
    public let requestedAt: Instant
}
