import AgencyProtocol
import Foundation

/// The answer to `POST /api/devices/changes`.
public struct DeviceChangeResult: Decodable, Equatable, Sendable {
    public enum Outcome: Decodable, Equatable, Sendable {
        /// This node holds the root key and wrote the change.
        case applied
        case delivered
        case queued
        /// The root key asks for its owner's presence: the root's node holds the change until the owner signs it there
        /// (`agency root-sign`). It is neither added nor refused.
        case pending
        /// An outcome this app does not know. It is kept, so that the answer still reads and is not taken for a success.
        case unknown(String)

        public init(from decoder: any Decoder) throws {
            let word = try decoder.singleValueContainer().decode(String.self)
            switch word {
            case "applied": self = .applied
            case "delivered": self = .delivered
            case "queued": self = .queued
            case "pending": self = .pending
            default: self = .unknown(word)
            }
        }
    }

    public let outcome: Outcome
    /// Why a change was queued, or what the node says of an outcome.
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
