import AgencyAPI
import Foundation

/// One line of the chart on a phone: the same tree as the web client draws, as an indented list.
public struct OrgChartRow: Equatable, Sendable, Identifiable {
    public enum Content: Equatable, Sendable {
        /// The box at the top: the account (every org), the org, or the focused department.
        case top(kind: String, name: String, sub: String?)
        case org(OrgNode)
        case department(DepartmentNode)
        case team(TeamNode)
        case person(PersonNode)
    }

    /// How deep the row sits under the top; the phone indents by it.
    public let depth: Int
    public let content: Content
    public let id: String

    /// The faces a team shows before saying how many more there are.
    public static let facesShown = 6

    /// The rows of the chart of every org, under the account.
    public static func allOrgs(account: String, orgs: [OrgNode]) -> [OrgChartRow] {
        let top = OrgChartRow(depth: 0, content: .top(kind: Ja.OrgChart.account, name: account, sub: nil), id: "top")
        return [top] + orgs.flatMap { node in
            [OrgChartRow(depth: 1, content: .org(node), id: "org:\(node.id)")] + rows(of: node.children, depth: 2, in: node.id)
        }
    }

    /// The rows of one org's chart, optionally focused on a department.
    public static func oneOrg(label: String, orgID: String, tree: OneOrgTree) -> [OrgChartRow] {
        let top: Content
        if let focus = tree.focus {
            top = .top(kind: Ja.OrgChart.department, name: focus.department.label, sub: focus.head.map { Ja.OrgChart.ledBy($0.label) } ?? Ja.OrgChart.noHead)
        } else {
            top = .top(kind: Ja.OrgChart.organization, name: label, sub: nil)
        }
        return [OrgChartRow(depth: 0, content: top, id: "top")] + rows(of: tree.children, depth: 1, in: orgID)
    }

    private static func rows(of children: [UnitChild], depth: Int, in org: String) -> [OrgChartRow] {
        children.flatMap { child -> [OrgChartRow] in
            switch child {
            case .department(let node):
                [OrgChartRow(depth: depth, content: .department(node), id: "\(org)/department:\(node.id)")] + rows(of: node.children, depth: depth + 1, in: org)
            case .team(let node):
                [OrgChartRow(depth: depth, content: .team(node), id: "\(org)/team:\(node.id)")]
            case .person(let node):
                [OrgChartRow(depth: depth, content: .person(node), id: "\(org)/person:\(node.id)")]
            }
        }
    }
}

/// How a team's box reads: the overview's signals in the overview's words.
public struct TeamBoxText: Equatable, Sendable {
    public let waiting: String?
    /// `止まっている` or `動いていない`, when the team is stuck or idle.
    public let tag: String?
    public let working: String?
    /// The team could not be read.
    public let unavailable: String?
    public let oldData: String?
    /// The faces: at most six, and how many more there are.
    public let faces: [AgentNode]
    public let more: String?

    public init(node: TeamNode, now: Date = Date()) {
        waiting = node.signals.flatMap { $0.mine > 0 ? Ja.Overview.waitingForYou($0.mine) : nil }
        tag = node.signals.flatMap { Ja.Overview.tag($0.attention) }.flatMap { node.signals?.attention == .mine ? nil : $0 }
        working = node.signals.map { $0.participants == 0 ? Ja.Overview.noParticipants : Ja.Overview.working($0.working, of: $0.participants) }
        unavailable = node.signals == nil && !node.isLoading ? Ja.OrgChart.teamUnavailable : nil
        oldData = node.team.flatMap { team in
            if case .replica(_, _, let error) = team.source, error != nil { Ja.OrgChart.oldData } else { nil }
        }
        faces = Array(node.agents.prefix(OrgChartRow.facesShown))
        more = node.agents.count > OrgChartRow.facesShown ? Ja.OrgChart.moreAgents(node.agents.count - OrgChartRow.facesShown) : nil
    }
}
