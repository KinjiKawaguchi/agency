import AgencyAPI
import Foundation
import Observation

/// Where this phone's push delivery is read from: the node's list of the owner's tokens. `NodeClient` is the real one.
public protocol PushTokenSource: Sendable {
    func pushTokens() async throws -> [RegisteredPushToken]
}

extension NodeClient: PushTokenSource {}

/// Whether APNs takes the notices the node sends this phone, for the Devices screen.
///
/// Nothing is said until the node has told, and nothing when APNs takes the notices, when the node has sent none, when this phone's
/// token is not among the listed ones, or when the node cannot be asked (an older node has no such facts, and a failed read leaves
/// what was known): a notice that is not there says nothing, and is not "all is well".
@MainActor
@Observable
public final class PushDeliveryStore {
    /// How long a read stays good before a screen that asks again is answered with it.
    public static let lifetime: TimeInterval = 30

    public private(set) var delivery: PushDelivery?

    @ObservationIgnored private let source: any PushTokenSource
    @ObservationIgnored private let ownToken: @MainActor () -> String?
    @ObservationIgnored private let now: @Sendable () -> Date
    @ObservationIgnored private var readAt: Date?

    /// - Parameter ownToken: this phone's push token, once Apple has given it one.
    public init(source: any PushTokenSource, ownToken: @escaping @MainActor () -> String?, now: @escaping @Sendable () -> Date = { Date() }) {
        self.source = source
        self.ownToken = ownToken
        self.now = now
    }

    /// What the Devices screen says, when it says anything.
    public var lines: [String]? { delivery?.lines() }

    /// Reads the node's list. `force` asks even when a read is still good (a pull to reload).
    public func refresh(force: Bool = false) async {
        let stamp = now()
        if !force, let readAt, stamp.timeIntervalSince(readAt) < Self.lifetime { return }
        guard let token = ownToken() else { return }
        do {
            let listed = try await source.pushTokens()
            readAt = stamp
            delivery = PushDelivery.ofOwn(token: token, in: listed)
        } catch {
            // An older node, a node that cannot say, or a read that failed: what was known stays, and nothing new is said.
        }
    }
}
