import Foundation

/// `GET /api/me`: the node's account and everything it can reach.
public struct Me: Decodable, Equatable, Sendable {
    public let account: AccountRef
    public let orgs: [OrgRef]
    public let funding: [Funding]
}

public struct AccountRef: Decodable, Equatable, Sendable {
    public let id: String
    public let label: String
    public let fingerprint: String
}

public struct OrgRef: Decodable, Equatable, Sendable {
    public let id: String
    public let label: String
    public let isRoot: Bool
    public let teams: [TeamRef]
}

public struct TeamRef: Decodable, Equatable, Sendable {
    public enum Location: String, Decodable, Sendable { case local, remote }

    public let id: String
    public let label: String
    public let location: Location
    /// The viewer's name in the team's runtime; absent when the team does not address them.
    public let viewer: String?
}

/// A source that pays for the account's agents, such as a subscription, with the usage windows that could be read.
public struct Funding: Decodable, Equatable, Sendable {
    public enum Kind: String, Decodable, Sendable { case subscription, apiKey }

    public let id: String
    public let label: String
    public let kind: Kind
    public let windows: [UsageWindow]
    public let readAt: Instant?
}

public struct UsageWindow: Decodable, Equatable, Sendable {
    public enum Name: String, Decodable, Sendable { case fiveHour, sevenDay }

    public let name: Name
    public let lengthHours: Int
    /// A rounded percent.
    public let utilization: Int
    public let resetsAt: Instant
}
