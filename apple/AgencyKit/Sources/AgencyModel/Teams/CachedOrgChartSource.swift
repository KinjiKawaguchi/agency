import AgencyAPI
import Foundation

/// What the org chart reads from: the account and the teams through the shared cache, and the orgs' definitions
/// straight from the client.
public struct CachedOrgChartSource<Client: OrgChartSource>: OrgChartSource {
    private let cache: SnapshotCache
    private let client: Client

    public init(cache: SnapshotCache, client: Client) {
        self.cache = cache
        self.client = client
    }

    public func me() async throws -> Me { try await cache.me() }
    public func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot { try await cache.teamSnapshot(org: org, team: team) }
    public func orgDefinition(org: String) async throws -> OrgDefinition { try await client.orgDefinition(org: org) }
}
