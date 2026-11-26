import Foundation

/// What a client read in one round: every team, the requests to add a device, and the decision log.
/// A part that could not be read is marked (`ok` false for a team, `nil` for a list), and the engine keeps
/// the last answer it had for it instead of taking the failure for an empty answer.
public struct NotificationReading: Equatable, Sendable {
    public struct Blocked: Equatable, Sendable {
        public let ref: String
        public let reason: String

        public init(ref: String, reason: String) {
            self.ref = ref
            self.reason = reason
        }
    }

    public struct TeamReading: Equatable, Sendable {
        public let org: String
        public let team: String
        public let ok: Bool
        /// The ids of the requests the node lists as the viewer's.
        public let mine: Set<String>
        /// The ids of the participants who wait on the viewer.
        public let waitingOnViewer: Set<String>
        public let blocked: [Blocked]

        public init(org: String, team: String, ok: Bool, mine: Set<String>, waitingOnViewer: Set<String>, blocked: [Blocked]) {
            self.org = org
            self.team = team
            self.ok = ok
            self.mine = mine
            self.waitingOnViewer = waitingOnViewer
            self.blocked = blocked
        }

        var key: String { "\(org)/\(team)" }
    }

    public struct Decision: Equatable, Sendable {
        public let id: String
        /// `queued`, `applied`, `delivered` or `refused`.
        public let state: String

        public init(id: String, state: String) {
            self.id = id
            self.state = state
        }
    }

    public let teams: [TeamReading]
    /// The public keys of the devices that asked to be added, or `nil` when that list could not be read.
    public let devices: [String]?
    /// The decision log, or `nil` when it could not be read.
    public let decisions: [Decision]?

    public init(teams: [TeamReading], devices: [String]?, decisions: [Decision]?) {
        self.teams = teams
        self.devices = devices
        self.decisions = decisions
    }
}

/// Who is reading, for the parts of "new" that depend on the reader.
public struct NotificationOwn: Equatable, Sendable {
    /// This client's own device key: a request from it is not news to it.
    public let deviceKey: String
    /// This client can approve other devices (it is added in some org). Without it, a request to add one is not news.
    public let canApprove: Bool
    /// The ids of the decisions this client sent: a success it caused is not news, a failure is.
    public let sentDecisionIds: Set<String>

    public init(deviceKey: String, canApprove: Bool, sentDecisionIds: Set<String>) {
        self.deviceKey = deviceKey
        self.canApprove = canApprove
        self.sentDecisionIds = sentDecisionIds
    }
}

public enum EventKind: Int, CaseIterable, Comparable, Sendable {
    /// A request now waits on the viewer.
    case request
    /// A participant now waits on the viewer.
    case participant
    /// A work item is newly blocked.
    case blocked
    /// A device asks to be added.
    case device
    /// A decision changed state.
    case decision

    public static func < (left: EventKind, right: EventKind) -> Bool { left.rawValue < right.rawValue }

    public var name: String {
        switch self {
        case .request: "request"
        case .participant: "participant"
        case .blocked: "blocked"
        case .device: "device"
        case .decision: "decision"
        }
    }
}

public struct NotificationEvent: Equatable, Sendable {
    public let kind: EventKind
    /// The stable key that tells two events apart: `request:org/team/id`, `participant:org/team/id`,
    /// `blocked:org/team/ref`, `device:<public key>`, `decision:id:state`.
    public let key: String

    public init(kind: EventKind, key: String) {
        self.kind = kind
        self.key = key
    }
}

/// The one definition of "what is new", shared with the web client (`testdata/notifications.json`).
///
/// An event is the difference between the last answer the engine kept and the next one. The first
/// answer for a team, the device list or the decision log is a baseline: nothing in it is new. A part
/// that could not be read keeps its last answer, so the read after a failure is not "everything is new".
public struct EventEngine: Sendable {
    private struct TeamMemory {
        var mine: Set<String>
        var waiting: Set<String>
        var blocked: Set<String>
    }

    private var teams: [String: TeamMemory] = [:]
    private var devices: Set<String>?
    private var decisions: [String: String]?

    public init() {}

    /// Takes a reading and says what is new since the last good ones, ordered by kind and then by key.
    public mutating func read(_ reading: NotificationReading, own: NotificationOwn) -> [NotificationEvent] {
        var events: [NotificationEvent] = []
        for team in reading.teams where team.ok {
            events += readTeam(team)
        }
        if let list = reading.devices { events += readDevices(Set(list), own: own) }
        if let log = reading.decisions { events += readDecisions(log, own: own) }
        return events.sorted(by: Self.isOrdered)
    }

    private mutating func readTeam(_ team: NotificationReading.TeamReading) -> [NotificationEvent] {
        let now = TeamMemory(mine: team.mine, waiting: team.waitingOnViewer, blocked: Set(team.blocked.map(\.ref)))
        defer { teams[team.key] = now }
        guard let before = teams[team.key] else { return [] }
        let prefix = team.key
        return now.mine.subtracting(before.mine).map { NotificationEvent(kind: .request, key: "request:\(prefix)/\($0)") }
            + now.waiting.subtracting(before.waiting).map { NotificationEvent(kind: .participant, key: "participant:\(prefix)/\($0)") }
            + now.blocked.subtracting(before.blocked).map { NotificationEvent(kind: .blocked, key: "blocked:\(prefix)/\($0)") }
    }

    private mutating func readDevices(_ keys: Set<String>, own: NotificationOwn) -> [NotificationEvent] {
        defer { devices = keys }
        guard let before = devices, own.canApprove else { return [] }
        return keys.subtracting(before).filter { $0 != own.deviceKey }.map { NotificationEvent(kind: .device, key: "device:\($0)") }
    }

    private mutating func readDecisions(_ log: [NotificationReading.Decision], own: NotificationOwn) -> [NotificationEvent] {
        let now = Dictionary(log.map { ($0.id, $0.state) }, uniquingKeysWith: { first, _ in first })
        defer { decisions = now }
        guard let before = decisions else { return [] }
        return now.compactMap { id, state in
            guard Self.isNews(id: id, state: state, before: before[id], own: own) else { return nil }
            return NotificationEvent(kind: .decision, key: "decision:\(id):\(state)")
        }
    }

    /// A known decision is news when its state changed. A new one is news when it is queued or refused,
    /// or when it succeeded and this client did not send it (a success it caused is not news).
    private static func isNews(id: String, state: String, before: String?, own: NotificationOwn) -> Bool {
        if let before { return before != state }
        if state == "queued" || state == "refused" { return true }
        return !own.sentDecisionIds.contains(id)
    }

    /// By kind, then by key in code point order (the same in every language).
    private static func isOrdered(_ left: NotificationEvent, _ right: NotificationEvent) -> Bool {
        if left.kind != right.kind { return left.kind < right.kind }
        return left.key.unicodeScalars.lexicographicallyPrecedes(right.key.unicodeScalars) { $0.value < $1.value }
    }
}
