import AgencyAPI
import Foundation

/// The groups of the pile, in display order.
public enum DeskGroup: Int, CaseIterable, Comparable, Sendable {
    case proposal, question, pendingDecision
    /// The viewer's requests of a kind this app does not know. The node says how to answer them (`answerWith`), so they can be.
    case unrecognized
    /// Requests addressed to someone else.
    case others

    public static func < (left: DeskGroup, right: DeskGroup) -> Bool { left.rawValue < right.rawValue }

    init(_ kind: Request.Kind) {
        switch kind {
        case .proposal: self = .proposal
        case .question: self = .question
        case .pendingDecision: self = .pendingDecision
        case .unknown: self = .unrecognized
        }
    }
}

/// One request on the desk.
public struct DeskItem: Equatable, Identifiable, Sendable {
    /// `<org>:<team>:<request id>`: unique across orgs and teams, the key of the item's draft, and the
    /// `item` of the deep link a notification carries.
    public let id: String
    public let group: DeskGroup
    public let request: Request
    public let org: String
    public let team: String
    public let teamLabel: String
    /// Absent when the request comes from something that is not a person on the team, such as the board.
    public let drafter: PersonStatus?
    /// The person it is addressed to: the viewer when it is theirs, otherwise whoever `to` names.
    public let decider: PersonStatus?
    /// Whether the node lists the request as the viewer's to decide. Only the viewer's can be decided here.
    public let isMine: Bool
    /// The people (not work refs) that are stopped until the request is decided.
    public let blockedPeople: [PersonStatus]
    /// The seat or external party the viewer answers this request for, when it is addressed to one (the node says so, for the
    /// viewer's own requests only).
    public let answersFor: String?

    /// Whether this app can make the envelope that answers it: the node names the kind of envelope (`answerWith`), and a kind
    /// this app does not know is one it cannot make. Such a request is shown and counted, and answered elsewhere.
    public var isAnswerableHere: Bool { request.answerWith.kind != .unknown }

    public static func key(org: String, team: String, request: String) -> String {
        "\(org):\(team):\(request)"
    }

    init(team: Team, request: Request) {
        let isMine = team.mine.contains(request.id)
        let addressee = isMine ? team.viewer : request.to
        id = Self.key(org: team.view.org, team: team.view.team, request: request.id)
        group = isMine ? DeskGroup(request.kind) : .others
        self.request = request
        org = team.view.org
        self.team = team.view.team
        teamLabel = team.view.label
        drafter = team.person(named: request.from)
        decider = addressee.flatMap(team.person(named:))
        self.isMine = isMine
        blockedPeople = request.blocks.compactMap(team.person(named:))
        // An empty label makes no line: the node does not send one, and a line that says nothing is not shown.
        answersFor = isMine ? team.relayedFor[request.id].flatMap { $0.isEmpty ? nil : $0 } : nil
    }
}

public enum Desk {
    /// Requests from every team, grouped, with the ones that stop the most people first and the
    /// oldest first within that.
    public static func openItems(_ teams: [Team]) -> [DeskItem] {
        let items = teams.flatMap { team in team.view.requests.map { DeskItem(team: team, request: $0) } }
        return items.enumerated().sorted { left, right in
            let (a, b) = (left.element, right.element)
            if a.group != b.group { return a.group < b.group }
            if a.blockedPeople.count != b.blockedPeople.count { return a.blockedPeople.count > b.blockedPeople.count }
            if a.request.createdAt != b.request.createdAt { return a.request.createdAt < b.request.createdAt }
            return left.offset < right.offset
        }.map(\.element)
    }

    /// The requests the viewer has to decide, across every team: the number on the desk's tab and on the app icon.
    public static func waitingCount(_ teams: [Team]) -> Int {
        openItems(teams).filter(\.isMine).count
    }
}
