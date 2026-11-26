import AgencyAPI
import Foundation

/// One window of a funding source, as the screen shows it.
public enum WindowReading: Equatable, Sendable {
    /// The node has no reading of this window.
    case notStarted(UsageWindow.Name)
    /// The last reading is from before the window's reset, so there is nothing to project.
    case stale(UsageWindow.Name)
    case projected(UsageWindow.Name, WindowForecast)

    public var name: UsageWindow.Name {
        switch self {
        case .notStarted(let name), .stale(let name), .projected(let name, _): name
        }
    }
}

/// A funding source with its windows, the weekly one first: it decides how much work fits.
public struct FundingSummary: Equatable, Sendable {
    public let id: String
    public let label: String
    public let kind: Funding.Kind
    /// Empty when the node could not read this source's usage; otherwise both windows.
    public let windows: [WindowReading]
    public let readAt: Instant?

    private static let order: [UsageWindow.Name] = [.sevenDay, .fiveHour]

    public init(funding: Funding, now: Date) {
        id = funding.id
        label = funding.label
        kind = funding.kind
        readAt = funding.readAt
        windows = funding.windows.isEmpty ? [] : Self.order.map { name in
            guard let reading = funding.windows.first(where: { $0.name == name }) else { return .notStarted(name) }
            return WindowForecast(reading: reading, now: now).map { .projected(name, $0) } ?? .stale(name)
        }
    }
}
