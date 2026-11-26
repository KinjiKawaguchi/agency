import AgencyKit
import Foundation
import Observation

/// Which screen is open, and where a banner or a tapped notification leads. Every shell (the iPhone's tab
/// bar, the Mac's sidebar) shows this and changes it; none decides where things lead on its own.
@MainActor
@Observable
public final class ShellRouter {
    public var tab: AppTab = .desk
    public var organizationPage: OrganizationPage = .orgChart
    /// The desk item to open on the desk (`<org>:<team>:<request>`), until the desk has taken it.
    public var openItemID: String?

    public init() {}

    /// The kinds of news the open screen shows by itself, so they make no banner: the devices screen lists a
    /// device that asks to be added.
    public var suppressedNewsKinds: Set<EventKind> {
        tab == .devices ? [.device] : []
    }

    /// A banner was tapped.
    public func open(_ banner: NewsBanner) {
        switch banner.destination {
        case .desk(let itemID):
            tab = .desk
            openItemID = itemID
        case .people:
            organizationPage = .people
            tab = .organization
        case .devices:
            tab = .devices
        }
    }

    /// A notification was tapped.
    public func open(_ link: NotificationLink) {
        switch link {
        case .request(let request):
            tab = .desk
            openItemID = DeskItem.key(org: request.org, team: request.team, request: request.request)
        case .devices:
            tab = .devices
        case .decision:
            tab = .desk
        }
    }
}
