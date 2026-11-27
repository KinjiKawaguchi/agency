import AgencyAPI
import AgencyKit
import AgencyModel
import Foundation

/// The decisions kept on this device for each home node, as a run with no window sees them: sent from their files with the Access token
/// the device holds. There is no session and no key: what is kept was signed when it was decided.
@MainActor
enum BackgroundOutbox {
    /// How long a woken app waits before it asks to be woken again for what a run did not get to.
    static let retryAfterCutShort: TimeInterval = 60

    /// The items that have a decision kept for `host`: the desk the widgets and the icon show leaves them out.
    static func decidedItemIDs(host: String) -> Set<String> {
        // A file that cannot be read now (protected until the first unlock) says nothing: the desk is then counted as the node gives it.
        guard let kept = try? FileOutboxStorage(host: host).load() else { return [] }
        return Set(kept.map(\.itemID))
    }

    /// Sends what is due for every home node of this device that has something kept, in `limit` at most, and returns when the earliest
    /// node that still has something to send wants another run; `nil` when none does.
    static func flush(defaults: UserDefaults = .standard, within limit: Duration = BackgroundDeskRefresh.timeLimit, now: Date = Date()) async -> Date? {
        let book = HomeNodeStore(defaults: defaults).book
        let nodes = [book.current].compactMap { $0 } + book.others.map(\.url)
        let retries = await BackgroundDeskRefresh.withTimeLimit(within: limit) { await flush(nodes) }
        // A run cut short did not get to the rest: they are tried a little later.
        guard let retries else { return now.addingTimeInterval(retryAfterCutShort) }
        return retries.min()
    }

    private static func flush(_ nodes: [HomeNodeURL]) async -> [Date] {
        var retries: [Date] = []
        for node in nodes {
            let storage = FileOutboxStorage(host: node.host)
            let tokens = AccessTokenStore(host: node.host)
            guard let kept = try? storage.load(), !kept.isEmpty, tokens.current() != nil else { continue }
            let client = NodeClient(
                transport: URLSessionTransport(homeNode: node), accessToken: { tokens.current()?.value }, clientIdentity: ClientIdentity.current()
            )
            let result = await BackgroundSending.run(storage: storage, submitter: client)
            retries.append(contentsOf: [result.retryAt].compactMap { $0 })
        }
        return retries
    }
}
