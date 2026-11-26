import Foundation

/// The kinds of notification the node sends to a closed app. A subset of the kinds of news: a participant
/// who waits and a work item that is blocked are states, not items, and are shown in the app only.
public enum PushKind: String, CaseIterable, Comparable, Sendable {
    case request
    case device
    case decision

    public static func < (left: PushKind, right: PushKind) -> Bool { left.sortOrder < right.sortOrder }

    private var sortOrder: Int { Self.allCases.firstIndex(of: self) ?? 0 }

    /// The kind of news this is the push for.
    public var eventKind: EventKind {
        switch self {
        case .request: .request
        case .device: .device
        case .decision: .decision
        }
    }
}

/// What the owner wants to be told, for one home node: the banners while the app is open (any kind of
/// news) and the pushes while it is closed (a `PushKind` each). Every kind starts on.
public struct NotificationPreferences: Equatable, Sendable {
    public var bannerKinds: Set<EventKind>
    public var pushKinds: Set<PushKind>

    public static let standard = NotificationPreferences(bannerKinds: Set(EventKind.allCases), pushKinds: Set(PushKind.allCases))

    public init(bannerKinds: Set<EventKind>, pushKinds: Set<PushKind>) {
        self.bannerKinds = bannerKinds
        self.pushKinds = pushKinds
    }

    /// The `events` the push token is registered with, in a fixed order: what the node sends this phone.
    public var pushEventNames: [String] { pushKinds.sorted().map(\.rawValue) }
}

extension NotificationPreferences: Codable {
    private enum CodingKeys: String, CodingKey { case banners, pushes }

    /// Names are stored, not positions, so a name that is no longer known is dropped and the rest are kept.
    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        let banners = try values.decode([String].self, forKey: .banners)
        let pushes = try values.decode([String].self, forKey: .pushes)
        bannerKinds = Set(EventKind.allCases.filter { banners.contains($0.name) })
        pushKinds = Set(PushKind.allCases.filter { pushes.contains($0.rawValue) })
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        try values.encode(EventKind.allCases.filter(bannerKinds.contains).map(\.name), forKey: .banners)
        try values.encode(pushEventNames, forKey: .pushes)
    }
}
