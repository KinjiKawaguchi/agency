import AgencyAPI
import Foundation

/// What the outcomes page reads from: the account through the shared cache, and the owner's numbers straight from the
/// client. The numbers are never put in the cache: they are business data and are read each time they are shown.
public struct CachedOutcomesSource<Client: OutcomesSource>: OutcomesSource {
    private let cache: SnapshotCache
    private let client: Client

    public init(cache: SnapshotCache, client: Client) {
        self.cache = cache
        self.client = client
    }

    public func me() async throws -> Me { try await cache.me() }
    public func outcomes(org: String, days: Int) async throws -> Outcomes { try await client.outcomes(org: org, days: days) }
    public func readings(org: String) async throws -> Readings { try await client.readings(org: org) }
}
