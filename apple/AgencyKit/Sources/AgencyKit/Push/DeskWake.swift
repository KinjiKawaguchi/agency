import Foundation

/// A push that wakes the app in the background to read the desk (agency#261): the node's push with only
/// `content-available`, or a request's alert, which carries it too. Both ask the same thing of the app.
public enum DeskWake {
    /// The least time between two readings. The node sends at most one wake in 15 minutes; this only keeps a
    /// burst (an alert and a wake together) from reading the desk twice.
    public static let minimumInterval: TimeInterval = 60

    /// Whether the payload asks the app to read the desk: `aps.content-available` is 1.
    public static func isWake(_ userInfo: [AnyHashable: Any]) -> Bool {
        guard let aps = userInfo["aps"] as? [String: Any] else { return false }
        // UIKit hands the flag over as an NSNumber, which reads as an Int.
        return (aps["content-available"] as? Int) == 1
    }

    /// Whether the last reading is old enough for another.
    public static func isDue(lastRun: Date?, now: Date) -> Bool {
        guard let lastRun else { return true }
        return now.timeIntervalSince(lastRun) >= minimumInterval
    }
}
