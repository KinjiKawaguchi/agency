import AgencyAPI
import Foundation

/// What the viewer needs to know about a person at a glance.
public enum Presence: Equatable, Sendable {
    /// The person can only continue after the viewer acts.
    case needsYou
    case working
    /// They wait on someone else.
    case waiting
    case idle
    case away

    /// `waitingOnViewer` is the node's list of the team's people who wait on the viewer. `unknown` counts as idle.
    public init(of person: PersonStatus, waitingOnViewer: Set<String>) {
        if waitingOnViewer.contains(person.id) {
            self = .needsYou
        } else if person.state == .away {
            self = .away
        } else if person.waitingOn != nil {
            self = .waiting
        } else {
            self = person.state == .working ? .working : .idle
        }
    }
}

/// The numbers behind the desk's one-line summary. The viewer is not counted among the people.
public struct Situation: Equatable, Sendable {
    public let waitingOnYou: Int
    public let working: Int
    public let total: Int
    /// Requests that are the viewer's to decide.
    public let mine: Int

    init(waitingOnYou: Int, working: Int, total: Int, mine: Int) {
        self.waitingOnYou = waitingOnYou
        self.working = working
        self.total = total
        self.mine = mine
    }

    public init(teams: [Team], items: [DeskItem]) {
        let others = teams.flatMap { team in
            team.view.people.filter { $0.id != team.viewer }.map { (person: $0, waiting: team.waitingOnViewer) }
        }
        self.init(
            waitingOnYou: others.filter { Presence(of: $0.person, waitingOnViewer: $0.waiting) == .needsYou }.count,
            working: others.filter { $0.person.state == .working }.count,
            total: others.count,
            mine: items.filter(\.isMine).count
        )
    }
}
