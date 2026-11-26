import Foundation

/// What a piece of work is for, as the board's "Work type" field or, failing that, the pull request's type says.
/// The work nothing could classify is `unclassified`, shown as it is and never guessed.
public enum EffortType: String, CaseIterable, Sendable {
    case newFeature, improvement, maintenance, debt, incident, unclassified

    /// The types a plan states a share for: everything but the unclassified work.
    public static let planned: [EffortType] = [.newFeature, .improvement, .maintenance, .debt, .incident]
}

/// `GET /api/orgs/<org>/effort?days=<1 to 90>`: where each team's tokens went, by work type. Read the way
/// the web client reads it: a type left out counts as 0, and a cost that is negative or not a number refuses
/// the whole answer.
public struct Effort: Decodable, Equatable, Sendable {
    public let org: String
    public let days: Int
    public let from: String
    public let until: String
    public let generatedAt: Instant
    public let teams: [TeamEffort]
    public let unavailable: [TeamUnavailable]
}

public struct TeamEffort: Decodable, Equatable, Sendable {
    public let team: String
    /// The period's first and last calendar day in the time zone of the team's node.
    public let from: String
    public let until: String
    public let byType: [EffortType: TypeUsage]
    /// Oldest day first; days with nothing in them may be left out.
    public let byDay: [DayEffort]

    private enum Keys: String, CodingKey { case team, from, until, byType, byDay }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        team = try container.decode(String.self, forKey: .team)
        from = try container.decode(String.self, forKey: .from)
        until = try container.decode(String.self, forKey: .until)
        let given = try container.nestedContainerIfPresent(keyedBy: AnyKey.self, forKey: .byType)
        var types: [EffortType: TypeUsage] = [:]
        for type in EffortType.allCases {
            types[type] = try given?.decodeIfPresent(TypeUsage.self, forKey: AnyKey(stringValue: type.rawValue)) ?? TypeUsage.none
        }
        byType = types
        byDay = try container.decodeIfPresent([DayEffort].self, forKey: .byDay) ?? []
    }
}

public struct TypeUsage: Decodable, Equatable, Sendable {
    /// Estimated cost in US dollars at API prices.
    public let costUsd: Double
    public let tokens: EffortTokens

    static let none = TypeUsage(costUsd: 0, tokens: EffortTokens(input: 0, output: 0, cacheRead: 0, cacheWrite: 0))

    init(costUsd: Double, tokens: EffortTokens) {
        self.costUsd = costUsd
        self.tokens = tokens
    }

    private enum Keys: String, CodingKey { case costUsd, tokens }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        costUsd = try container.amount(.costUsd)
        tokens = try container.decodeIfPresent(EffortTokens.self, forKey: .tokens) ?? EffortTokens(input: 0, output: 0, cacheRead: 0, cacheWrite: 0)
    }
}

public struct EffortTokens: Decodable, Equatable, Sendable {
    public let input: Int
    public let output: Int
    public let cacheRead: Int
    public let cacheWrite: Int

    init(input: Int, output: Int, cacheRead: Int, cacheWrite: Int) {
        self.input = input
        self.output = output
        self.cacheRead = cacheRead
        self.cacheWrite = cacheWrite
    }

    private enum Keys: String, CodingKey { case input, output, cacheRead, cacheWrite }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        input = Int(try container.amount(.input))
        output = Int(try container.amount(.output))
        cacheRead = Int(try container.amount(.cacheRead))
        cacheWrite = Int(try container.amount(.cacheWrite))
    }
}

/// One calendar day in the time zone of the team's node.
public struct DayEffort: Decodable, Equatable, Sendable {
    public let date: String
    public let costUsd: [EffortType: Double]

    private enum Keys: String, CodingKey { case date, costUsd }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        date = try container.decode(String.self, forKey: .date)
        let given = try container.nestedContainerIfPresent(keyedBy: AnyKey.self, forKey: .costUsd)
        var costs: [EffortType: Double] = [:]
        for type in EffortType.allCases {
            costs[type] = try given?.optionalAmount(AnyKey(stringValue: type.rawValue)) ?? 0
        }
        costUsd = costs
    }

    public init(date: String, costUsd: [EffortType: Double]) {
        self.date = date
        self.costUsd = EffortType.allCases.reduce(into: [:]) { $0[$1] = costUsd[$1] ?? 0 }
    }
}

extension KeyedDecodingContainer {
    func nestedContainerIfPresent<NestedKey: CodingKey>(keyedBy type: NestedKey.Type, forKey key: Key) throws -> KeyedDecodingContainer<NestedKey>? {
        guard contains(key), try !decodeNil(forKey: key) else { return nil }
        return try nestedContainer(keyedBy: type, forKey: key)
    }
}

// MARK: What the org intends

/// What the org intends for a team: the share of its effort for each work type (whole numbers that sum to 100)
/// and its budget for a week, in whole US dollars at API prices. It is `teams[].plan` of the signed org definition.
public struct TeamPlan: Equatable, Sendable {
    public let allocation: [EffortType: Int]
    public let weeklyBudgetUsd: Int

    public init(allocation: [EffortType: Int], weeklyBudgetUsd: Int) {
        self.allocation = allocation
        self.weeklyBudgetUsd = weeklyBudgetUsd
    }
}

/// The plans of an org's teams, read from `GET /api/orgs/<org>` (the org definition). The definition is the
/// node's and the root key's, so only a plan with the shape the protocol allows is kept: whole shares that sum
/// to 100, and a whole budget. Anything else is "no plan stated", never a guess.
public struct OrgPlans: Decodable, Equatable, Sendable {
    /// The plan of each team that has a valid one, by team id.
    public let byTeam: [String: TeamPlan]

    public init(byTeam: [String: TeamPlan]) {
        self.byTeam = byTeam
    }

    private enum Keys: String, CodingKey { case teams }
    private struct Team: Decodable {
        let id: String?
        let plan: Plan?
        init(from decoder: any Decoder) throws {
            let container = try decoder.container(keyedBy: AnyKey.self)
            id = try? container.decodeIfPresent(String.self, forKey: AnyKey(stringValue: "id"))
            plan = try? container.decodeIfPresent(Plan.self, forKey: AnyKey(stringValue: "plan"))
        }
    }
    private struct Plan: Decodable {
        let allocation: [String: Double]?
        let weeklyBudgetUsd: Double?
        init(from decoder: any Decoder) throws {
            let container = try decoder.container(keyedBy: AnyKey.self)
            allocation = try? container.decodeIfPresent([String: Double].self, forKey: AnyKey(stringValue: "allocation"))
            weeklyBudgetUsd = try? container.decodeIfPresent(Double.self, forKey: AnyKey(stringValue: "weeklyBudgetUsd"))
        }
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        let teams = (try? container.decodeIfPresent([Team].self, forKey: .teams)) ?? []
        var plans: [String: TeamPlan] = [:]
        for team in teams {
            if let id = team.id, let plan = team.plan.flatMap(Self.valid) { plans[id] = plan }
        }
        byTeam = plans
    }

    private static func isCount(_ value: Double) -> Int? {
        value >= 0 && value == value.rounded() && value.isFinite ? Int(value) : nil
    }

    private static func valid(_ plan: Plan) -> TeamPlan? {
        guard let allocation = plan.allocation, let budget = plan.weeklyBudgetUsd.flatMap(isCount) else { return nil }
        var shares: [EffortType: Int] = [:]
        for type in EffortType.planned {
            guard let share = allocation[type.rawValue].flatMap(isCount) else { return nil }
            shares[type] = share
        }
        guard shares.values.reduce(0, +) == 100 else { return nil }
        return TeamPlan(allocation: shares, weeklyBudgetUsd: budget)
    }
}
