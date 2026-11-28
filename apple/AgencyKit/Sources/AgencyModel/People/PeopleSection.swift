import AgencyAPI
import Foundation

/// How many held refs a participant's card shows before the rest fold away, and how many rows a work list shows.
public enum PeopleLimits {
    public static let heldShown = 6
    public static let rowsShown = 8

    /// The items to draw, and how many are folded away.
    public static func shown<Item>(_ items: [Item], limit: Int, isExpanded: Bool) -> (items: [Item], hidden: Int) {
        let shown = isExpanded ? items : Array(items.prefix(limit))
        return (shown, items.count - shown.count)
    }
}

/// One participant on a team: what they are doing, what they hold, and what they wait on the viewer for.
public struct ParticipantCard: Equatable, Sendable, Identifiable {
    public let person: PersonStatus
    public let presence: Presence
    /// The title of the first request that stops this participant, when they wait on the viewer.
    public let waitingForTitle: String?
    public var id: String { person.id }
    public var isLead: Bool { person.role == .lead }
    public var stateTag: String? { Ja.People.stateTag(presence) }
    public var since: Date? { person.since?.date }

    init(person: PersonStatus, team: Team) {
        self.person = person
        presence = Presence(of: person, waitingOnViewer: team.waitingOnViewer)
        waitingForTitle = presence == .needsYou ? team.view.requests.first { $0.blocks.contains(person.id) }?.title : nil
    }
}

/// A work item as one row: who holds it, why it is stuck, and when it should be done.
public struct WorkRow: Equatable, Sendable, Identifiable {
    public let ref: String
    public let title: String
    public let assigneeLabel: String
    public let assigneeKind: PersonStatus.Kind
    /// `相談待ち 11月23日 から`, for work that cannot move.
    public let blockedText: String?
    /// The forecast, for work in progress.
    public let etaText: String?
    /// How the forecast was made, for a long press.
    public let etaBasis: String?
    public var id: String { ref }

    init(item: WorkItem, view: TeamView) {
        let assignee = item.assignee.flatMap { name in view.people.first { $0.id == name } }
        ref = item.ref
        title = item.title
        assigneeLabel = assignee?.label ?? item.assignee ?? Ja.People.unassigned
        assigneeKind = assignee?.kind ?? .agent
        blockedText = item.blocked.map { "\(Ja.Work.blockedReason($0.reason)) \(Ja.People.blockedSince(JaFormat.shortDate($0.since)))" }
        etaText = item.status == .inProgress ? Self.eta(item.eta) : nil
        etaBasis = item.status == .inProgress ? item.eta?.basis : nil
    }

    private static func eta(_ eta: Eta?) -> String {
        guard let eta else { return Ja.People.etaNone }
        if eta.isOverBudget { return Ja.People.etaOverBudget(eta.basis) }
        return "\(Ja.People.etaAround(JaFormat.dateTime(eta.p50.date)))、\(Ja.People.etaLatest(JaFormat.dateTime(eta.p80.date)))"
    }
}

/// An epic with its progress bar and forecast.
public struct EpicRow: Equatable, Sendable, Identifiable {
    public let ref: String
    public let title: String
    /// `1 / 4`.
    public let count: String
    public let segments: [Work.Segment]
    public let barLabel: String
    /// When it should finish, or why there is no forecast.
    public let meta: String
    public let blockedBy: String?
    public let basis: String
    /// The epic is done: it shows when, and no forecast.
    public let isFinished: Bool
    public var id: String { ref }

    init(epic: EpicForecast) {
        ref = epic.ref
        title = epic.title
        count = "\(epic.done) / \(epic.total)"
        segments = Work.segments(of: epic)
        barLabel = Ja.People.epicBar(total: epic.total, done: epic.done, doing: epic.doing, blocked: Work.blockedTotal(epic))
        isFinished = epic.isFinished
        if epic.isFinished {
            meta = epic.doneAt.map { Ja.People.epicFinishedAt(JaFormat.dateTime($0.date)) } ?? Ja.People.epicFinished
        } else {
            meta = epic.forecast.map { Ja.People.epicFinish(JaFormat.dateTime($0.at.date)) } ?? epic.basis
        }
        let reasons = epic.blocked.map { "\(Ja.Work.blockedReason($0.reason)) \($0.count)" }.joined(separator: "、")
        blockedBy = reasons.isEmpty ? nil : Ja.People.epicBlockedBy(reasons)
        basis = epic.basis
    }
}

/// One team on the participants and work screen: its participants without the viewer, the work, the epics.
public struct PeopleSection: Equatable, Sendable, Identifiable {
    public let team: Team
    public let cards: [ParticipantCard]
    public let working: Int
    public let waitingOnViewer: Int
    public let doing: [WorkRow]
    public let blocked: [WorkRow]
    public let epics: [EpicRow]

    public var id: String { team.key }
    public var title: String { Ja.People.team(team.view.label) }

    /// How many are working, and, when any wait on the viewer, how many.
    public var summary: String {
        let line = Ja.People.summary(working: working, of: cards.count)
        return waitingOnViewer > 0 ? "\(line) \(Ja.People.waitingOnYou(waitingOnViewer))" : line
    }

    public init(team: Team) {
        self.team = team
        let view = team.view
        cards = view.people.filter { $0.id != team.viewer }.map { ParticipantCard(person: $0, team: team) }
        working = cards.filter { $0.person.state == .working }.count
        waitingOnViewer = cards.filter { $0.presence == .needsYou }.count
        doing = Work.inProgress(view.work).map { WorkRow(item: $0, view: view) }
        blocked = Work.blocked(view.work).map { WorkRow(item: $0, view: view) }
        epics = Work.sorted(view.epics).map(EpicRow.init)
    }
}

/// What a team's copy says about its age, in the words the desk uses; `nil` for a team on this node.
public enum ReplicaNotice {
    public static func text(for team: Team, now: Date) -> String? {
        guard case .replica(let fetchedAt, _, let error) = team.source else { return nil }
        let reason = error ?? "—"
        guard let fetchedAt else { return Ja.Desk.neverRead(reason: reason) }
        return Ja.Desk.staleCopy(when: Ago.text(from: fetchedAt.date, now: now), reason: reason)
    }
}
