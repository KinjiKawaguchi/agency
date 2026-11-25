import AgencyAPI
import Foundation

/// A team as the screens use it: what the team reported, and who the viewer is in it.
public struct Team: Equatable, Sendable {
    public let view: TeamView
    /// The team runtime's name for the person the viewer answers for; `nil` when the team does not address the viewer.
    public let viewer: String?
    public let source: TeamSnapshot.Source
    /// The ids of the requests that are the viewer's to decide: the node's rule, never worked out here.
    public let mine: Set<String>
    /// The ids of the team's people who wait on the viewer: the node's rule, never worked out here.
    public let waitingOnViewer: Set<String>

    public init(snapshot: TeamSnapshot) {
        view = snapshot.view
        viewer = snapshot.viewer
        source = snapshot.source
        mine = Set(snapshot.mine)
        waitingOnViewer = Set(snapshot.waitingOnViewer)
    }

    public var isReplica: Bool {
        if case .replica = source { true } else { false }
    }

    /// `org/team`, unique across the orgs the node can reach.
    public var key: String { "\(view.org)/\(view.team)" }

    /// The person a team runtime calls `name`, if the team lists them.
    public func person(named name: String) -> PersonStatus? {
        view.people.first { $0.id == name }
    }
}
