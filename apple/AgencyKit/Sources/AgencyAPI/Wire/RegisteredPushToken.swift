import AgencyProtocol
import Foundation

/// A push token the node has registered. The node shows only the last characters of a token it lists.
public struct RegisteredPushToken: Decodable, Equatable, Sendable {
    public enum Environment: String, OpenWireEnum { case production, sandbox, unknown }

    public let token: String
    public let environment: Environment
    public let label: String
    /// The kinds of notification this token gets (`request`, `device`, `decision`). A node that predates kinds
    /// does not say, and sends requests only.
    public let events: [String]?
    public let registeredAt: Instant
}
