import AgencyProtocol
import Foundation

/// The answer to `POST /api/devices/changes`.
public struct DeviceChangeResult: Decodable, Equatable, Sendable {
    public enum Outcome: String, Decodable, Sendable {
        /// This node holds the root key and wrote the change.
        case applied
        case delivered
        case queued
    }

    public let outcome: Outcome
    /// Why a change was queued.
    public let message: String?

    public init(outcome: Outcome, message: String?) {
        self.outcome = outcome
        self.message = message
    }
}

/// A device change signed by one of the person's devices.
public struct SignedDeviceChange: Sendable {
    public let change: DeviceChange
    /// The id of the signing device in the change's org.
    public let signer: String
    /// The signature in standard base64.
    public let signature: String

    public init(change: DeviceChange, signer: String, signature: String) {
        self.change = change
        self.signer = signer
        self.signature = signature
    }

    var json: JSONValue {
        .object([.init("body", change.json), .init("signer", .string(signer)), .init("signature", .string(signature))])
    }
}
