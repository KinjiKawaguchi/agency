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
    /// When APNs last took a notice for the token; left out until it has.
    public let lastSucceededAt: Instant?
    /// The last time APNs did not, and why: its status and its reason code, or that it could not be reached, never any other words of
    /// it. Left out until there was one. Both are kept when the app registers the same token again.
    public let lastFailure: PushFailure?
}

/// A notice APNs did not take: when, and the reason the node can give without APNs' own words.
public struct PushFailure: Decodable, Equatable, Sendable {
    public let at: Instant
    public let reason: String
}

/// `GET /api/push/tokens`: the owner's registered tokens, each shortened to its last 8 characters.
public struct PushTokenList: Decodable, Equatable, Sendable {
    public let tokens: [RegisteredPushToken]
}
