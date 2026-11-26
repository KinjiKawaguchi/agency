import Observation
import SwiftUI

/// How many requests wait for the owner, for a place outside the window that shows it (the Mac's menu-bar item).
/// `nil` before the desk was read, and when no window has a session.
@MainActor
@Observable
final class WaitingStatus {
    private(set) var count: Int?

    func set(_ count: Int?) {
        self.count = count
    }
}

private struct WaitingStatusKey: EnvironmentKey {
    static let defaultValue: WaitingStatus? = nil
}

extension EnvironmentValues {
    /// Where the shell reports how many requests wait, when the platform has a place to show it.
    var waitingStatus: WaitingStatus? {
        get { self[WaitingStatusKey.self] }
        set { self[WaitingStatusKey.self] = newValue }
    }
}
