import AgencyAPI
import Foundation

/// What tells the node this phone's push token.
public protocol PushTokenPoster: Sendable {
    @discardableResult
    func registerPushToken(token: String, environment: String, label: String, events: [String]?) async throws -> RegisteredPushToken
}

extension NodeClient: PushTokenPoster {}

/// Registers this phone's push token with the kinds of notification it wants.
///
/// A node that predates kinds refuses a registration that names `events` (it answers 400 to a field it does
/// not know), and sends requests only. For that node the registration is made again without them, so the
/// phone still gets requests, and the answer says the kinds were not taken.
public struct PushRegistrar: Sendable {
    public enum Outcome: Equatable, Sendable {
        /// The node took the kinds.
        case withKinds
        /// The node is older than kinds: it sends requests only.
        case requestsOnly
    }

    private let poster: any PushTokenPoster

    public init(poster: any PushTokenPoster) {
        self.poster = poster
    }

    public func register(token: String, environment: String, label: String, events: [String]) async throws -> Outcome {
        do {
            try await poster.registerPushToken(token: token, environment: environment, label: label, events: events)
            return .withKinds
        } catch NodeError.http(let status, _) where status == 400 {
            try await poster.registerPushToken(token: token, environment: environment, label: label, events: nil)
            return .requestsOnly
        }
    }
}
