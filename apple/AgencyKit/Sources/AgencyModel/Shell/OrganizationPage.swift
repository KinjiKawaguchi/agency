import Foundation

/// The two pages of the organization tab.
public enum OrganizationPage: CaseIterable, Hashable, Sendable {
    case orgChart, people

    public var title: String {
        switch self {
        case .orgChart: Ja.Tabs.orgChart
        case .people: Ja.Tabs.people
        }
    }
}
