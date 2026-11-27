import AgencyAPI
import Foundation

/// What the node says the viewer is to it, which decides what the app offers: a member the node hosts (a guest) has their decisions and their
/// devices, and anyone else has the owner's screens. As the web's (`useStanding`): the viewer's screens are not the owner's until the node
/// has said who is asking, and a read that failed with nothing earlier to go by counts as anyone else, so that the screens show their own
/// failures.
public enum Standing: Equatable, Sendable {
    /// The node has not said yet.
    case loading
    /// The read of the account failed and nothing was known before it.
    case unknown
    case member
    case guest(GuestRef)

    public init(me: Me) {
        self = me.guest.map(Standing.guest) ?? .member
    }

    public var isGuest: Bool {
        if case .guest = self { true } else { false }
    }
}

extension AppTab {
    /// A member the node hosts has two screens: their decisions, and their devices; so has a viewer the node has not told yet, who is not
    /// offered the owner's screens for a moment. Anyone else has all of them.
    public static func tabs(for standing: Standing) -> [AppTab] {
        switch standing {
        case .loading, .guest: [.desk, .devices]
        case .unknown, .member: allCases
        }
    }
}
