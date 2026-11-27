import AgencyAPI
import Foundation

/// What tells the node this phone's push token.
public protocol PushTokenPoster: Sendable {
    @discardableResult
    func registerPushToken(
        token: String, environment: String, label: String, events: [String]?, device: String?
    ) async throws -> RegisteredPushToken
}

extension NodeClient: PushTokenPoster {}

/// Registers this phone's push token with the kinds of notification it wants.
///
/// A node that predates kinds refuses a registration that names `events` (it answers 400 to a field it does
/// not know), and sends requests only. For that node the registration is made again without them, so the
/// phone still gets requests, and the answer says the kinds were not taken. A registration that names its `device` is never made again
/// without it: only a node that lists `pushDevice` is sent one, so a 400 for it is a device that is not the account's, not an old node,
/// and a registration that went through without the device would not be tied to it.
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

    public func register(token: String, environment: String, label: String, events: [String], device: String? = nil) async throws -> Outcome {
        do {
            try await poster.registerPushToken(token: token, environment: environment, label: label, events: events, device: device)
            return .withKinds
        } catch NodeError.http(let status, _) where status == 400 && device == nil {
            try await poster.registerPushToken(token: token, environment: environment, label: label, events: nil, device: nil)
            return .requestsOnly
        }
    }
}
