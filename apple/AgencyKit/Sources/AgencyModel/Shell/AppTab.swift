import Foundation

/// The screens of the app, in the order the tab bar (iPhone) and the sidebar (Mac) list them.
public enum AppTab: CaseIterable, Hashable, Sendable {
    case desk, overview, organization, results, devices

    public var title: String {
        switch self {
        case .desk: Ja.Tabs.desk
        case .overview: Ja.Tabs.overview
        case .organization: Ja.Tabs.organization
        case .results: Ja.Tabs.results
        case .devices: Ja.Tabs.devices
        }
    }

    public var symbol: String {
        switch self {
        case .desk: "tray.full"
        case .overview: "square.grid.2x2"
        case .organization: "rectangle.3.group"
        case .results: "chart.line.uptrend.xyaxis"
        case .devices: "iphone"
        }
    }
}
