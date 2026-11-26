import Foundation

/// `GET /api/orgs/<org>/teams/<team>/usage?days=N`: what a team's agents used over the last `days` days.
public struct TokenUsage: Decodable, Equatable, Sendable {
    public let org: String
    public let team: String
    public let days: Int
    public let generatedAt: Instant
    /// Oldest day first; days without use may be left out.
    public let byDay: [DayCost]
    public let byPerson: [PersonUsage]
}

public struct DayCost: Decodable, Equatable, Sendable {
    /// A calendar date, `YYYY-MM-DD`.
    public let date: String
    /// Estimated cost in US dollars at API prices, by the participant's name in the team's runtime.
    public let costUsd: [String: Double]
}

public struct PersonUsage: Decodable, Equatable, Sendable {
    /// The participant's name in the team's runtime.
    public let person: String
    public let costUsd: Double
    public let tokens: TokenCounts
}

public struct TokenCounts: Decodable, Equatable, Sendable {
    public let input: Int
    public let output: Int
    public let cacheRead: Int
    public let cacheWrite: Int
}
