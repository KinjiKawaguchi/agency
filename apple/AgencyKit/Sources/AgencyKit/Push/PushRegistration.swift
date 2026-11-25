import Foundation

/// The APNs host a device token belongs to. A token from one environment is refused by the other.
public enum PushEnvironment: String, Codable, Equatable, Sendable {
    case sandbox
    case production

    /// Maps the value of the `aps-environment` entitlement.
    public init?(apsEnvironment: String) {
        switch apsEnvironment {
        case "development": self = .sandbox
        case "production": self = .production
        default: return nil
        }
    }
}

/// An APNs device token, written as the node expects it: lowercase hex.
public struct PushToken: Equatable, Sendable {
    public let hex: String

    public init?(deviceToken: Data) {
        guard !deviceToken.isEmpty else { return nil }
        hex = deviceToken.reduce(into: "") { text, byte in
            text += byte < 16 ? "0" : ""
            text += String(byte, radix: 16)
        }
    }
}

/// What the page sends to the node to register this device for push notifications.
public struct PushRegistration: Codable, Equatable, Sendable {
    public let token: String
    public let environment: PushEnvironment
    public let label: String

    public init(token: PushToken, environment: PushEnvironment, label: String) {
        self.token = token.hex
        self.environment = environment
        self.label = label
    }
}
