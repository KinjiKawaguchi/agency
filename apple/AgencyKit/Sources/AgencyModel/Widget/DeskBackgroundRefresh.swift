import AgencyAPI
import Foundation

/// Reads the owner's desk once, with no screen and no session, and writes what the widgets show: what a woken
/// app does when the node says the desk changed while the app was closed (agency#261).
///
/// It holds nothing between runs. A run that cannot read the whole desk writes nothing: a number made of the
/// teams that happened to answer would be wrong, and the widget already says how old the file it has is.
@MainActor
public struct DeskBackgroundRefresh {
    public enum Outcome: Equatable, Sendable {
        /// The widgets' file was written from a complete reading of the desk, which had this many requests waiting for the owner.
        case written(waiting: Int)
        /// The desk could not be read in full (the sign-in ended, the node did not answer, a team did not). Nothing was written.
        case notRead
    }

    private let source: any TeamSnapshotSource
    private let publisher: WidgetPublisher

    public init(source: any TeamSnapshotSource, publisher: WidgetPublisher) {
        self.source = source
        self.publisher = publisher
    }

    /// One reading: the account, every team once (no waiting for a team that is not ready: a background run has
    /// seconds), and the file. `excluding` are the items the owner has already decided and the node has not taken yet: they are not on
    /// the desk, as in the app.
    public func run(now: Date = Date(), excluding decided: Set<String>) async -> Outcome {
        guard let (me, teams) = await DeskReading.complete(from: source) else { return .notRead }
        publisher.preferences.nodeSays(hidesTitles: me.hidesTitles)
        publisher.publish(items: Desk.openItems(teams, excluding: decided), now: now)
        return .written(waiting: Desk.waitingCount(teams, excluding: decided))
    }
}
