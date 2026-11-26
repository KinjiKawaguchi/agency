import AgencyAPI
import Foundation

/// What the evaluation reads from: the account through the shared cache, and the evaluation, the effort and
/// the org definitions' plans straight from the client.
public struct CachedEvaluationSource<Client: EvaluationSource>: EvaluationSource {
    private let cache: SnapshotCache
    private let client: Client

    public init(cache: SnapshotCache, client: Client) {
        self.cache = cache
        self.client = client
    }

    public func me() async throws -> Me { try await cache.me() }
    public func evaluation(org: String, days: Int) async throws -> Evaluation { try await client.evaluation(org: org, days: days) }
    public func effort(org: String, days: Int) async throws -> Effort { try await client.effort(org: org, days: days) }
    public func orgPlans(org: String) async throws -> OrgPlans { try await client.orgPlans(org: org) }
}
