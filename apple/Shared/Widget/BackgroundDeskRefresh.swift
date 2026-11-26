import AgencyAPI
import AgencyKit
import AgencyModel
import Foundation

/// What a woken app does with no window: reads the desk as the saved home node and signed-in account, and writes the
/// widgets' file. Nothing here touches the device key or a screen.
@MainActor
enum BackgroundDeskRefresh {
    /// The time a background run has before the system takes the app back is about 30 seconds; this stops
    /// reading a little before.
    static let timeLimit: Duration = .seconds(25)

    private static let lastRunKey = "agency.backgroundRefreshAt"

    /// Reads the desk and writes the file, unless a reading just happened, there is no home node, or nobody is signed in.
    /// `nil` means there was nothing to do (no home node, nobody signed in, or a reading just happened).
    static func run(defaults: UserDefaults = .standard, now: Date = Date()) async -> DeskBackgroundRefresh.Outcome? {
        let lastRun = defaults.object(forKey: lastRunKey) as? Date
        guard DeskWake.isDue(lastRun: lastRun, now: now) else { return nil }
        guard let homeNode = HomeNodeStore(defaults: defaults).homeNode else { return nil }
        let tokens = AccessTokenStore(host: homeNode.host)
        // Nobody signed in is not a failed read: there is nothing to refresh, as with no home node.
        guard tokens.current() != nil else { return nil }
        defaults.set(now, forKey: lastRunKey)
        let client = NodeClient(
            transport: URLSessionTransport(homeNode: homeNode), accessToken: { tokens.current()?.value }, clientIdentity: ClientIdentity.current()
        )
        let preferences = WidgetPreferencesStore(storage: WidgetDefaultsStorage(host: homeNode.host))
        let publisher = WidgetPublisher(host: homeNode.host, preferences: preferences, sink: WidgetAppGroupSink())
        let refresh = DeskBackgroundRefresh(source: client, publisher: publisher)
        return await withTimeLimit { await refresh.run(now: now) } ?? .notRead
    }

    /// The result of `work`, or `nil` when it did not finish in `timeLimit`.
    private static func withTimeLimit<Value: Sendable>(_ work: @escaping @MainActor @Sendable () async -> Value) async -> Value? {
        await withTaskGroup(of: Value?.self) { group in
            group.addTask { await work() }
            group.addTask {
                try? await Task.sleep(for: timeLimit)
                return nil
            }
            let first = await group.next() ?? nil
            group.cancelAll()
            return first
        }
    }
}
