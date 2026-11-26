import AgencyAPI
import Foundation
import Observation

/// What the news reads from, each time it asks: the account, every team's view, the devices and the decision log.
public protocol NewsSource: TeamSnapshotSource {
    func devices() async throws -> DeviceList
    func decisions(limit: Int) async throws -> DecisionLog
}

extension NodeClient: NewsSource {}

/// The account and the teams through the shared cache, the rest straight from the client.
public struct CachedNewsSource<Client: NewsSource>: NewsSource {
    private let cache: SnapshotCache
    private let client: Client

    public init(cache: SnapshotCache, client: Client) {
        self.cache = cache
        self.client = client
    }

    public func me() async throws -> Me { try await cache.me() }
    public func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot { try await cache.teamSnapshot(org: org, team: team) }
    public func devices() async throws -> DeviceList { try await client.devices() }
    public func decisions(limit: Int) async throws -> DecisionLog { try await client.decisions(limit: limit) }
}

/// Where tapping a banner goes.
public enum NewsDestination: Equatable, Sendable {
    /// The desk, with this item open when there is one (`<org>:<team>:<request>`).
    case desk(itemID: String?)
    case people
    case devices
}

/// What the banner shows for one kind of event: one line, how many events it stands for, and where it leads.
public struct NewsBanner: Equatable, Identifiable, Sendable {
    public let kind: EventKind
    public let text: String
    public let count: Int
    public let destination: NewsDestination
    public var id: String { kind.name }
}

/// The news: what is new since the last time the app read the node, as banners (agency#167).
///
/// Each `poll()` reads the account, every team, the devices and the decision log, gives the reading to
/// the shared definition of "what is new" (`EventEngine`), and turns the events of the reading into at most
/// three banners, one for each kind, worded for one event or for several. A kind the owner turned off, or
/// one the visible screen already shows, makes no banner (its events are still taken, so they are not
/// announced later).
@MainActor
@Observable
public final class NewsStore {
    /// How many banners a reading makes at most.
    public static let maxBanners = 3
    public static let historyLimit = 20

    public private(set) var banners: [NewsBanner] = []
    /// The kinds the owner wants to see a banner for.
    public var enabledKinds: Set<EventKind> = Set(EventKind.allCases)
    /// The kinds the visible screen already shows by itself, such as the devices while the devices screen is open.
    public var suppressedKinds: Set<EventKind> = []

    @ObservationIgnored private let source: any NewsSource
    @ObservationIgnored private let own: @MainActor () -> NotificationOwn
    @ObservationIgnored private var engine = EventEngine()
    @ObservationIgnored private var snapshots: [String: TeamSnapshot] = [:]
    @ObservationIgnored private var devices: DeviceList?
    @ObservationIgnored private var decisions: DecisionLog?

    /// - Parameter own: who is reading: this phone's device key, whether it can approve other devices, and
    ///   the decisions it sent; asked at each reading, because they change.
    public init(source: any NewsSource, own: @escaping @MainActor () -> NotificationOwn) {
        self.source = source
        self.own = own
    }

    /// Reads once and makes the banners for what is new. A read that fails is no news, and is not asked again
    /// here, because the next poll is soon. The engine keeps the last answer of that part.
    public func poll() async {
        guard let me = try? await source.me() else { return }
        var teams: [NotificationReading.TeamReading] = []
        await TeamReader.read(me.orgs, from: source, retry: .never) { key, result in
            let parts = key.split(separator: "/", maxSplits: 1).map(String.init)
            guard parts.count == 2 else { return }
            switch result {
            case .success(let snapshot):
                snapshots[key] = snapshot
                teams.append(Self.reading(of: snapshot, org: parts[0], team: parts[1]))
            case .failure:
                teams.append(NotificationReading.TeamReading(org: parts[0], team: parts[1], ok: false, mine: [], waitingOnViewer: [], blocked: []))
            }
        }
        let list = try? await source.devices()
        let log = try? await source.decisions(limit: Self.historyLimit)
        if let list { devices = list }
        if let log { decisions = log }
        let reading = NotificationReading(
            teams: teams, devices: list?.requests.map(\.publicKey),
            decisions: log?.entries.map { .init(id: $0.id, state: Self.stateName($0.state)) }
        )
        let events = engine.read(reading, own: own())
        banners = makeBanners(events)
    }

    /// Takes the banners away, when the owner closes them or they have been shown long enough.
    public func dismiss() {
        banners = []
    }

    // MARK: The reading

    private static func reading(of snapshot: TeamSnapshot, org: String, team: String) -> NotificationReading.TeamReading {
        NotificationReading.TeamReading(
            org: org, team: team, ok: true, mine: Set(snapshot.mine), waitingOnViewer: Set(snapshot.waitingOnViewer),
            blocked: snapshot.view.work.compactMap { item in item.blocked.map { .init(ref: item.ref, reason: $0.reason.rawValue) } }
        )
    }

    private static func stateName(_ state: DecisionState) -> String {
        switch state {
        case .applied: "applied"
        case .delivered: "delivered"
        case .queued: "queued"
        case .refused: "refused"
        case .unknown: "unknown"
        }
    }

    // MARK: The banners

    private func makeBanners(_ events: [NotificationEvent]) -> [NewsBanner] {
        let shown = Set(EventKind.allCases).intersection(enabledKinds).subtracting(suppressedKinds)
        let grouped = Dictionary(grouping: events.filter { shown.contains($0.kind) }, by: \.kind)
        return EventKind.allCases.compactMap { kind in grouped[kind].map { banner(kind, $0) } }.prefix(Self.maxBanners).map { $0 }
    }

    private func banner(_ kind: EventKind, _ events: [NotificationEvent]) -> NewsBanner {
        let first = events[0]
        let text: String
        switch kind {
        case .request: text = events.count == 1 ? requestText(first) : Ja.News.requests(events.count)
        case .participant: text = events.count == 1 ? participantText(first) : Ja.News.participants(events.count)
        case .blocked: text = events.count == 1 ? blockedText(first) : Ja.News.blockedMany(events.count)
        case .device: text = events.count == 1 ? deviceText(first) : Ja.News.devices(events.count)
        case .decision: text = events.count == 1 ? decisionText(first) : Ja.News.decisions(events.count)
        }
        return NewsBanner(kind: kind, text: text, count: events.count, destination: destination(kind, first, count: events.count))
    }

    /// Where a banner leads: one new request opens that request; several open the desk, where they are.
    private func destination(_ kind: EventKind, _ event: NotificationEvent, count: Int) -> NewsDestination {
        switch kind {
        case .request:
            guard count == 1, let parts = Self.parts(of: event) else { return .desk(itemID: nil) }
            return .desk(itemID: DeskItem.key(org: parts.org, team: parts.team, request: parts.rest))
        case .participant, .blocked: return .people
        case .device: return .devices
        case .decision: return .desk(itemID: nil)
        }
    }

    /// `kind:org/team/rest`
    private static func parts(of event: NotificationEvent) -> (org: String, team: String, rest: String)? {
        let afterKind = event.key.dropFirst(event.kind.name.count + 1)
        let pieces = afterKind.split(separator: "/", maxSplits: 2, omittingEmptySubsequences: false).map(String.init)
        return pieces.count == 3 ? (pieces[0], pieces[1], pieces[2]) : nil
    }

    private func requestText(_ event: NotificationEvent) -> String {
        guard let parts = Self.parts(of: event), let snapshot = snapshots["\(parts.org)/\(parts.team)"] else { return Ja.News.requests(1) }
        let title = snapshot.view.requests.first { $0.id == parts.rest }?.title ?? Ja.News.unknownTitle
        return Ja.News.request(team: snapshot.view.label, title: title)
    }

    private func participantText(_ event: NotificationEvent) -> String {
        guard let parts = Self.parts(of: event), let snapshot = snapshots["\(parts.org)/\(parts.team)"] else { return Ja.News.participants(1) }
        return Ja.News.participant(label: snapshot.view.people.first { $0.id == parts.rest }?.label ?? parts.rest)
    }

    private func blockedText(_ event: NotificationEvent) -> String {
        guard let parts = Self.parts(of: event), let snapshot = snapshots["\(parts.org)/\(parts.team)"],
              let item = snapshot.view.work.first(where: { $0.ref == parts.rest })
        else { return Ja.News.blockedMany(1) }
        return Ja.News.blocked(team: snapshot.view.label, title: item.title, reason: item.blocked.map { Ja.Work.blockedReason($0.reason) } ?? "")
    }

    private func deviceText(_ event: NotificationEvent) -> String {
        let key = String(event.key.dropFirst("device:".count))
        return Ja.News.device(label: devices?.requests.first { $0.publicKey == key }?.label ?? "")
    }

    private func decisionText(_ event: NotificationEvent) -> String {
        let pieces = event.key.dropFirst("decision:".count).split(separator: ":", omittingEmptySubsequences: false).map(String.init)
        guard pieces.count == 2 else { return Ja.News.decisions(1) }
        let entry = decisions?.entries.first { $0.id == pieces[0] }
        let title = entry?.title.flatMap { $0.isEmpty ? nil : $0 }
        let reason: String? = if case .refused(let message)? = entry?.state { message } else { nil }
        return Ja.News.decision(title: title, state: pieces[1], reason: reason)
    }
}
