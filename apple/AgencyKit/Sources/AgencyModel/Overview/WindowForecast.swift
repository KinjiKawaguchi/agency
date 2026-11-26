import AgencyAPI
import Foundation

public enum WindowStatus: Equatable, Sendable {
    /// Projected to end well short of full: the window will go partly unused.
    case waste
    case ok
    /// Projected to reach the limit before the reset.
    case overrun
}

/// A usage window projected to its reset from the pace so far.
public struct WindowForecast: Equatable, Sendable {
    public let utilization: Int
    public let resetsAt: Date
    /// Percent expected at the reset if the pace so far holds.
    public let projectedAtReset: Double
    /// When the limit is reached, if that happens before the reset.
    public let hitAt: Date?
    public let status: WindowStatus

    /// Below this share of the window there is too little to extrapolate from.
    static let minimumElapsedShare = 0.02
    /// A window projected to end below this percent counts as going unused.
    public static let wasteBelowPercent = 85.0

    /// Projects a window using its own length. `nil` when the reset has already passed, which means the
    /// reading is out of date and says nothing about the current window, or when the length is not positive.
    public init?(reading: UsageWindow, now: Date) {
        let length = TimeInterval(reading.lengthHours) * 3600
        let resetsAt = reading.resetsAt.date
        let remaining = resetsAt.timeIntervalSince(now)
        guard remaining > 0, length > 0 else { return nil }

        let utilization = Double(reading.utilization)
        let elapsed = min(length, max(0, length - remaining))
        let hasEnoughElapsed = elapsed >= length * Self.minimumElapsedShare
        let projected = hasEnoughElapsed && utilization > 0 ? utilization * length / elapsed : utilization
        let isFull = utilization >= 100
        let willRunOut = isFull || projected > 100

        self.utilization = reading.utilization
        self.resetsAt = resetsAt
        projectedAtReset = projected
        hitAt = !isFull && willRunOut ? resetsAt.addingTimeInterval(-length + elapsed * 100 / utilization) : nil
        status = willRunOut ? .overrun : (projected < Self.wasteBelowPercent && hasEnoughElapsed ? .waste : .ok)
    }
}
