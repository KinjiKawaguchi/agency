import Foundation

/// `GET /api/me`: the node's account and everything it can reach.
public struct Me: Decodable, Equatable, Sendable {
    public let account: AccountRef
    public let orgs: [OrgRef]
    public let funding: [Funding]
    /// The node keeps titles and labels off what it sends to devices (`hideTitles` in its `push` setting), because its data
    /// must stay where it is. A client that shows a title outside the app (a widget) then shows none. Left out by a node that
    /// does not say, which counts as `false`.
    public let hidesTitles: Bool
    /// The owner's other home nodes, as this node's configuration names them. Only the node's owner is told; for anyone
    /// else, and for a node that names none, the list is empty.
    public let homeNodes: [HomeNodeRef]
    /// The id this node puts in its pushes (`agency.node`), as the node works it out from its own address; absent from a node
    /// that has none or does not say. An app maps a push to a node by it.
    public let nodeId: String?

    private enum Keys: String, CodingKey { case account, orgs, funding, hidesTitles, homeNodes, nodeId }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        account = try container.decode(AccountRef.self, forKey: .account)
        orgs = try container.decode([OrgRef].self, forKey: .orgs)
        funding = try container.decode([Funding].self, forKey: .funding)
        hidesTitles = try container.decodeIfPresent(Bool.self, forKey: .hidesTitles) ?? false
        homeNodes = try container.decodeIfPresent([HomeNodeRef].self, forKey: .homeNodes) ?? []
        nodeId = try container.decodeIfPresent(String.self, forKey: .nodeId)
    }
}

/// A home node a node's configuration names: a label and the address as the node writes it (`home_node_origin`).
public struct HomeNodeRef: Decodable, Equatable, Sendable {
    public let label: String
    public let url: String

    public init(label: String, url: String) {
        self.label = label
        self.url = url
    }
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
