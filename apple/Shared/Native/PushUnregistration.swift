import AgencyAPI
import AgencyKit
import Foundation

/// Tells a home node to stop sending to this device's push token, before the device stops signing in to it or forgets it:
/// afterwards the node could not be told, and its alerts would keep arriving and each would set the icon number to that node's
/// count. The node's own list of devices is not touched.
enum PushUnregistration {
    /// How long the owner waits for the node before it is said that it could not be told.
    static let patience = Duration.seconds(10)

    /// Whether the node no longer sends to `token` (also when this device has no token to stop); `false` when it could not be
    /// told: not signed in there, not reachable, or no answer in time.
    static func stop(token: String?, at node: HomeNodeURL) async -> Bool {
        guard let token else { return true }
        let tokens = AccessTokenStore(host: node.host)
        let client = NodeClient(
            transport: URLSessionTransport(homeNode: node), accessToken: { tokens.current()?.value }, clientIdentity: ClientIdentity.current()
        )
        return await withTaskGroup(of: Bool.self) { group in
            group.addTask { (try? await client.unregisterPushToken(token)) != nil }
            group.addTask {
                try? await Task.sleep(for: patience)
                return false
            }
            let first = await group.next() ?? false
            group.cancelAll()
            return first
        }
    }
}
