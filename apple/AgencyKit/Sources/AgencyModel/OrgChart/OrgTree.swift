import AgencyAPI
import Foundation

/// A participant of a team, drawn as a face.
public struct AgentNode: Equatable, Sendable, Identifiable {
    public let person: PersonStatus
    public let presence: Presence
    public let isLead: Bool
    public var id: String { person.id }
}

/// A team in the chart: where it is placed, what it reported, and what the overview says of it.
public struct TeamNode: Equatable, Sendable, Identifiable {
    public let place: TeamPlace
    /// Absent while the team is being read, and when it could not be read.
    public let team: Team?
    /// The team is being read (and asked again if it is not ready): its box shows a skeleton, not a failure.
    public let isLoading: Bool
    public let agents: [AgentNode]
    public let working: Int
    /// What the overview says of the team: requests waiting on the viewer, blocked work, who is working. Absent when the team could not be read.
    public let signals: TeamSummary?
    public var id: String { place.id }
}

/// A member of the org, and whether it is the viewer's own seat.
public struct PersonNode: Equatable, Sendable, Identifiable {
    public let member: OrgMember
    public let isYou: Bool
    public var id: String { member.id }
}

public struct DepartmentNode: Equatable, Sendable, Identifiable {
    public let department: OrgDepartment
    public let head: OrgMember?
    public let children: [UnitChild]
    /// Requests that are the viewer's anywhere below.
    public let mine: Int
    public var id: String { department.id }
}

/// What hangs under an org or a department: its departments, then its teams, then its members.
public indirect enum UnitChild: Equatable, Sendable, Identifiable {
    case department(DepartmentNode)
    case team(TeamNode)
    case person(PersonNode)

    public var id: String {
        switch self {
        case .department(let node): "department:\(node.id)"
        case .team(let node): "team:\(node.id)"
        case .person(let node): "person:\(node.id)"
        }
    }

    var mine: Int {
        switch self {
        case .department(let node): node.mine
        case .team(let node): node.signals?.mine ?? 0
        case .person: 0
        }
    }
}

/// An org in the chart of every org: what the node says of it, and its units.
public struct OrgNode: Equatable, Sendable, Identifiable {
    public enum State: Equatable, Sendable { case ready, loading, failed }

    public let org: OrgRef
    /// Empty while the org's definition is loading or when it could not be read.
    public let children: [UnitChild]
    public let mine: Int
    public let state: State
    public var id: String { org.id }
}

/// The chart of one org, optionally focused on a department.
public struct OneOrgTree: Equatable, Sendable {
    /// The department the chart is focused on, or `nil` for the whole organization.
    public let focus: DepartmentNode?
    /// What hangs directly under the top box.
    public let children: [UnitChild]
}

/// An org's definition and whether reading it failed.
public struct OrgInput: Equatable, Sendable {
    public let org: OrgRef
    /// The org's signed definition, once read.
    public let definition: OrgDefinition?
    public let failed: Bool

    public init(org: OrgRef, definition: OrgDefinition?, failed: Bool) {
        self.org = org
        self.definition = definition
        self.failed = failed
    }
}

/// Builds the charts from the definitions and the teams' views, ported from `web/src/org/tree.ts`.
public enum OrgTreeBuilder {
    /// The `org/team` keys of the teams that are still being read.
    public typealias Loading = Set<String>

    private static func teamNode(_ place: TeamPlace, orgID: String, teams: [Team], loading: Loading) -> TeamNode {
        guard let found = teams.first(where: { $0.view.org == orgID && $0.view.team == place.id }) else {
            return TeamNode(place: place, team: nil, isLoading: loading.contains(TeamReader.key(org: orgID, team: place.id)), agents: [], working: 0, signals: nil)
        }
        let agents = found.view.people.filter { $0.kind == .agent }.map {
            AgentNode(person: $0, presence: Presence(of: $0, waitingOnViewer: found.waitingOnViewer), isLead: $0.role == .lead)
        }
        return TeamNode(
            place: place, team: found, isLoading: false, agents: agents,
            working: agents.filter { $0.person.state == .working }.count, signals: TeamSummary(team: found)
        )
    }

    /// A department's departments, teams and members, in that order. `nil` is the top of the org.
    private static func children(of parent: String?, _ definition: OrgDefinition, teams: [Team], viewerAccount: String, loading: Loading) -> [UnitChild] {
        let departments = definition.departments.filter { $0.parent == parent }.map { department -> UnitChild in
            let below = children(of: department.id, definition, teams: teams, viewerAccount: viewerAccount, loading: loading)
            return .department(DepartmentNode(
                department: department, head: definition.people.first { $0.id == department.head }, children: below, mine: below.reduce(0) { $0 + $1.mine }
            ))
        }
        let places = definition.teams.filter { $0.department == parent }.map {
            UnitChild.team(teamNode($0, orgID: definition.org, teams: teams, loading: loading))
        }
        let people = definition.people.filter { $0.department == parent }.map {
            UnitChild.person(PersonNode(member: $0, isYou: $0.account == viewerAccount))
        }
        return departments + places + people
    }

    private static func findDepartment(_ children: [UnitChild], id: String) -> DepartmentNode? {
        for case .department(let node) in children {
            if node.id == id { return node }
            if let inside = findDepartment(node.children, id: id) { return inside }
        }
        return nil
    }

    /// The chart of an org, optionally focused on one department and everything below it.
    public static func oneOrg(_ definition: OrgDefinition, teams: [Team], viewerAccount: String, focus focusID: String? = nil, loading: Loading = []) -> OneOrgTree {
        let whole = children(of: nil, definition, teams: teams, viewerAccount: viewerAccount, loading: loading)
        let focus = focusID.flatMap { findDepartment(whole, id: $0) }
        return OneOrgTree(focus: focus, children: focus?.children ?? whole)
    }

    /// Every org of the account side by side, each with its departments, teams and members, and the signals of the overview.
    public static func allOrgs(_ orgs: [OrgInput], teams: [Team], viewerAccount: String, loading: Loading = []) -> [OrgNode] {
        orgs.map { input in
            guard let definition = input.definition else {
                return OrgNode(org: input.org, children: [], mine: 0, state: input.failed ? .failed : .loading)
            }
            let below = children(of: nil, definition, teams: teams, viewerAccount: viewerAccount, loading: loading)
            return OrgNode(org: input.org, children: below, mine: below.reduce(0) { $0 + $1.mine }, state: .ready)
        }
    }
}
