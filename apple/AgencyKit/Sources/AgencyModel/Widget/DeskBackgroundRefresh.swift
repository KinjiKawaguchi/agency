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
        /// The widgets' file was written from a complete reading of the desk.
        case written
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
    /// seconds), and the file.
    public func run(now: Date = Date()) async -> Outcome {
        guard let me = try? await source.me() else { return .notRead }
        let expected = me.orgs.reduce(0) { $0 + $1.teams.count }
        var teams: [Team] = []
        await TeamReader.read(me.orgs, from: source, retry: .never) { _, result in
            if case .success(let snapshot) = result { teams.append(Team(snapshot: snapshot)) }
        }
        // A team that failed, or whose read was cancelled, is missing from `teams`: that is an incomplete desk.
        guard teams.count == expected else { return .notRead }
        publisher.preferences.nodeSays(hidesTitles: me.hidesTitles)
        publisher.publish(items: Desk.openItems(teams.sorted { $0.key < $1.key }), now: now)
        return .written
    }
}
