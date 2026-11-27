import AgencyAPI
import AgencyKit
import AgencyModel
import Foundation

/// What a woken app does with no window: reads the desk of the node the wake names (the open node when it names none), keeps that
/// node's count on the icon, and, for the open node, writes the widgets' file. Nothing here touches the device key or a screen.
@MainActor
enum BackgroundDeskRefresh {
    /// The time a background run has before the system takes the app back is about 30 seconds; this stops
    /// reading a little before.
    static let timeLimit: Duration = .seconds(25)

    private static let lastRunKey = "agency.backgroundRefreshAt"

    /// Reads the desk and records what it says, unless a reading of that node just happened, it is not a node of this device, or
    /// nobody is signed in to it. `nil` means there was nothing to do.
    /// - Parameter node: the id the wake names (`agency.node`); none for a node that does not name itself.
    static func run(node: String? = nil, defaults: UserDefaults = .standard, now: Date = Date()) async -> DeskBackgroundRefresh.Outcome? {
        let book = HomeNodeStore(defaults: defaults).book
        guard let current = book.current else { return nil }
        let target = WakeTarget.resolve(node: node, current: current, others: book.others.map(\.url), idOf: book.nodeID(of:))
        let homeNode: HomeNodeURL
        switch target {
        case .current: homeNode = current
        case .other(let other): homeNode = other
        case .unknown: return nil
        }
        // One reading at a time for each node: a wake of another node is not held back by the open node's.
        let key = "\(lastRunKey).\(book.nodeID(of: homeNode))"
        guard DeskWake.isDue(lastRun: defaults.object(forKey: key) as? Date, now: now) else { return nil }
        let tokens = AccessTokenStore(host: homeNode.host)
        // What the owner decided and the node has not taken yet is not on the desk: the icon and the widgets leave it out, as the app does.
        let decided = BackgroundOutbox.decidedItemIDs(host: homeNode.host)
        // Nobody signed in is not a failed read: there is nothing to refresh, as with no home node.
        guard tokens.current() != nil else { return nil }
        defaults.set(now, forKey: key)
        let client = NodeClient(
            transport: URLSessionTransport(homeNode: homeNode), accessToken: { tokens.current()?.value }, clientIdentity: ClientIdentity.current()
        )
        let outcome: DeskBackgroundRefresh.Outcome
        if case .current = target {
            outcome = await readOpenNode(homeNode, client: client, now: now, excluding: decided)
        } else {
            // The widgets show the open node: another node's wake keeps only its count.
            outcome = await withTimeLimit { await DeskWaitingCount(source: client).run(excluding: decided) }.flatMap { $0 }
                .map { .written(waiting: $0) } ?? .notRead
        }
        if case .written(let waiting) = outcome { BadgeKeeper().record(waiting, for: book.nodeID(of: homeNode)) }
        return outcome
    }

    private static func readOpenNode(
        _ homeNode: HomeNodeURL, client: NodeClient, now: Date, excluding decided: Set<String>
    ) async -> DeskBackgroundRefresh.Outcome {
        let preferences = WidgetPreferencesStore(storage: WidgetDefaultsStorage(host: homeNode.host))
        let publisher = WidgetPublisher(host: homeNode.host, preferences: preferences, sink: WidgetAppGroupSink())
        let refresh = DeskBackgroundRefresh(source: client, publisher: publisher)
        return await withTimeLimit { await refresh.run(now: now, excluding: decided) } ?? .notRead
    }

    /// The result of `work`, or `nil` when it did not finish in `timeLimit`.
    static func withTimeLimit<Value: Sendable>(
        within limit: Duration = timeLimit, _ work: @escaping @MainActor @Sendable () async -> Value
    ) async -> Value? {
        await withTaskGroup(of: Value?.self) { group in
            group.addTask { await work() }
            group.addTask {
                try? await Task.sleep(for: limit)
                return nil
            }
            let first = await group.next() ?? nil
            group.cancelAll()
            return first
        }
    }
}
