import AgencyProtocol
import Foundation

/// `GET /api/orgs/<org>/teams/<team>/budget`: where this week's spend stands against the plan's weekly budget, as the node works it out
/// (`docs/protocol.md`, "Budget"). The app compares nothing: the week, the thresholds and the state are the node's.
public struct BudgetStatus: Decodable, Equatable, Sendable {
    /// The Monday of the node's week, `YYYY-MM-DD`.
    public let weekStart: String
    public let team: BudgetPart
    public let org: BudgetPart

    public init(weekStart: String, team: BudgetPart, org: BudgetPart) {
        self.weekStart = weekStart
        self.team = team
        self.org = org
    }
}

/// The budget of a team, or of the org's planned teams together.
public struct BudgetPart: Decodable, Equatable, Sendable {
    public enum State: String, OpenWireEnum {
        case ok, warning, over
        /// A state a newer node names: said as no state, never taken for `ok`.
        case unknown
    }

    /// The weekly budget of the plan in whole US dollars; `nil` for a team with no plan (the org's: when no team has one).
    public let capUsd: Int?
    /// The cost of the week at API prices. A required field: a missing spend must not read as nothing spent.
    public let spentUsd: Double
    public let state: State
    /// The usage could not be read: for the team, its `spentUsd` is 0 and says nothing; for the org, `spentUsd` is a lower bound.
    public let isPartial: Bool

    private enum Keys: String, CodingKey { case capUsd, spentUsd, state, partial }

    public init(capUsd: Int?, spentUsd: Double, state: State, isPartial: Bool = false) {
        self.capUsd = capUsd
        self.spentUsd = spentUsd
        self.state = state
        self.isPartial = isPartial
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        capUsd = try container.decodeIfPresent(Int.self, forKey: .capUsd)
        spentUsd = try container.decode(Double.self, forKey: .spentUsd)
        state = try container.decode(State.self, forKey: .state)
        // Left out when false, and only ever `true`.
        isPartial = try container.decodeIfPresent(Bool.self, forKey: .partial) == true
    }
}
