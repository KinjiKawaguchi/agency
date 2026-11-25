import Crypto
import Foundation

/// The fingerprint of a public key: the first 16 bytes of the SHA-256 of the key's bytes, as hex.
/// Invite links carry it, and a device's pairing code is its first 12 characters.
public struct DeviceFingerprint: Equatable, Sendable {
    public let hex: String

    public init(publicKey: Data) {
        hex = SHA256.hash(data: publicKey).prefix(16).map { String(format: "%02x", $0) }.joined()
    }

    public init(key: DevicePublicKey) {
        self.init(publicKey: key.bytes)
    }

    /// `dbc2-9825-1c51`: 48 bits in groups of 4, which the person compares on both devices.
    public var pairingCode: String {
        let digits = Array(hex.prefix(12))
        return stride(from: 0, to: 12, by: 4).map { String(digits[$0..<$0 + 4]) }.joined(separator: "-")
    }

    /// A device's id is `d-` and its pairing code without the dashes.
    public var deviceID: String {
        "d-" + pairingCode.replacingOccurrences(of: "-", with: "")
    }
}
