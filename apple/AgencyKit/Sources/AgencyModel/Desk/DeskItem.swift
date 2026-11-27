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
    /// Who drafted it, as a screen draws it: the person's label when the team lists them, else the runtime's name without the
    /// characters that can reorder a line. Finding the person goes by the name as written (`team.person(named:)`), never by this.
    public var drafterName: String { drafter?.label ?? request.shownFrom }
    /// Who wrote each earlier message of the thread, drawn the same way as the drafter and in the same order as `request.thread`: the
    /// person's label when the team lists them, else the runtime's name shown.
    public let threadAuthors: [String]
    /// Who it is addressed to, drawn the same way; `nil` when it names no one.
    public var deciderName: String? { decider?.label ?? request.shownTo }
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

    /// A label as a line shows it: without the spaces around it, and none at all when nothing else is left (the node does not send
    /// an empty one, and a line that says nothing is not shown).
    public static func shownLabel(_ label: String) -> String? {
        let trimmed = label.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

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
        threadAuthors = (request.thread ?? []).map { message in team.person(named: message.from)?.label ?? message.shownFrom }
        decider = addressee.flatMap(team.person(named:))
        self.isMine = isMine
        blockedPeople = request.blocks.compactMap(team.person(named:))
        answersFor = isMine ? team.relayedFor[request.id].flatMap(Self.shownLabel) : nil
    }
}

public enum Desk {
    /// Requests from every team, grouped, with the ones that stop the most people first and the
    /// oldest first within that.
    /// - Parameter decided: the items the owner has decided and the node has not yet taken (`DecisionOutbox.decidedItemIDs`): they are
    ///   not waiting for the owner any more, so they are left out until the node's own view agrees.
    public static func openItems(_ teams: [Team], excluding decided: Set<String> = []) -> [DeskItem] {
        let items = teams.flatMap { team in team.view.requests.map { DeskItem(team: team, request: $0) } }.filter { !decided.contains($0.id) }
        return items.enumerated().sorted { left, right in
            let (a, b) = (left.element, right.element)
            if a.group != b.group { return a.group < b.group }
            if a.blockedPeople.count != b.blockedPeople.count { return a.blockedPeople.count > b.blockedPeople.count }
            if a.request.createdAt != b.request.createdAt { return a.request.createdAt < b.request.createdAt }
            return left.offset < right.offset
        }.map(\.element)
    }

    /// The item to open after `id` was decided: the next of the viewer's own that this app can answer, in the pile's order, and when
    /// there is none after it, the first from the top; `nil` when nothing else waits for them.
    public static func next(after id: String, in items: [DeskItem]) -> String? {
        let answerable = items.filter { $0.isMine && $0.isAnswerableHere && $0.id != id }
        guard let position = items.firstIndex(where: { $0.id == id }) else { return answerable.first?.id }
        return answerable.first { candidate in items.firstIndex(where: { $0.id == candidate.id }).map { $0 > position } ?? false }?.id
            ?? answerable.first?.id
    }

    /// The requests the viewer has to decide, across every team: the number on the desk's tab and on the app icon.
    public static func waitingCount(_ teams: [Team], excluding decided: Set<String> = []) -> Int {
        openItems(teams, excluding: decided).filter(\.isMine).count
    }
}
