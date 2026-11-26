import Foundation

/// One request a widget lists: its title and team to read, and the key that opens it in the app.
public struct WidgetRequest: Codable, Equatable, Sendable {
    public enum Kind: String, Codable, Sendable { case proposal, question, pendingDecision, other }

    /// The most characters of a title the snapshot keeps: a widget has room for a line or two.
    public static let titleLimit = 80
    public static let teamLimit = 40

    /// The org, the team and the request id, which is what opens the request in the app.
    public let org: String
    public let team: String
    public let request: String
    public let kind: Kind
    /// What the request is about; `nil` when titles are not to be shown outside the app (the owner turned them off on
    /// widgets, or the node hides them), and then nothing of the request but its kind is in the file.
    public let title: String?
    /// The team's label, left out with the title.
    public let teamLabel: String?

    public init(org: String, team: String, request: String, kind: Kind, title: String?, teamLabel: String?) {
        self.org = org
        self.team = team
        self.request = request
        self.kind = kind
        self.title = title.map { Self.cut($0, to: Self.titleLimit) }
        self.teamLabel = teamLabel.map { Self.cut($0, to: Self.teamLimit) }
    }

    /// Cut at a character boundary, with an ellipsis when something was left out.
    static func cut(_ text: String, to limit: Int) -> String {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.count > limit else { return trimmed }
        return String(trimmed.prefix(limit - 1)) + "…"
    }
}

/// What a widget shows of the desk, written by the app after it reads and read by the widget.
///
/// The widget never holds the Access token or a device key: this small file in the app group is all it
/// has. It carries the number of requests that wait on the viewer and the first few of them, and nothing
/// else (no people, no bodies, no options).
public struct WidgetSnapshot: Codable, Equatable, Sendable {
    /// Bumped when the shape changes in a way an older reader cannot read; a file of another version is ignored.
    public static let currentVersion = 1
    /// How many requests a snapshot lists.
    public static let requestLimit = 3

    public let version: Int
    public let writtenAt: Date
    /// The home node the snapshot is about.
    public let host: String
    /// Requests that are the viewer's to decide, across every team: the number on the desk's tab.
    public let waitingCount: Int
    /// The first of them, in the desk's order.
    public let requests: [WidgetRequest]

    public init(host: String, writtenAt: Date, waitingCount: Int, requests: [WidgetRequest]) {
        version = Self.currentVersion
        self.host = host
        self.writtenAt = writtenAt
        self.waitingCount = max(waitingCount, 0)
        self.requests = Array(requests.prefix(Self.requestLimit))
    }

    /// Whether `other` says the same to a person: the same node, count and requests, whenever it was written.
    /// The app writes only when this is false (or the file has aged), so a widget is not reloaded for nothing.
    public func hasSameContent(as other: WidgetSnapshot) -> Bool {
        host == other.host && waitingCount == other.waitingCount && requests == other.requests
    }

    /// How out of date the snapshot is: the app writes after each read, so an old one means the app has not run.
    public enum Staleness: Equatable, Sendable {
        case fresh
        /// Older than `staleAfter`: the widget says how long ago.
        case stale(minutes: Int)
    }

    public static let staleAfter: TimeInterval = 3 * 3600

    public func staleness(now: Date) -> Staleness {
        let age = now.timeIntervalSince(writtenAt)
        return age < Self.staleAfter ? .fresh : .stale(minutes: Int(age / 60))
    }
}
