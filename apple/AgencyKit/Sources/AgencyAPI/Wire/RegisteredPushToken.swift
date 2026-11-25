import Foundation

/// A push token the node has registered. The node shows only the last characters of a token it lists.
public struct RegisteredPushToken: Decodable, Equatable, Sendable {
    public enum Environment: String, Decodable, Sendable { case production, sandbox }

    public let token: String
    public let environment: Environment
    public let label: String
    public let registeredAt: Instant
}
