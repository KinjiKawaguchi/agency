import AgencyProtocol
import Foundation

/// `GET /api/orgs/<org>/evaluation?days=<1 to 90>`: how fast each team of an org ships.
///
/// The node works the numbers out and the screen only draws them. They are read the way the web client reads
/// them: a type the node leaves out counts as 0, and a number that is negative or not a number refuses the
/// whole answer, so a screen can rely on every type being there.
public struct Evaluation: Decodable, Equatable, Sendable {
    public let org: String
    public let days: Int
    /// The period's first and last calendar day in the time zone of the node that answers. A team on a node in
    /// another time zone has its own `from` and `until`.
    public let from: String
    public let until: String
    public let generatedAt: Instant
    public let teams: [TeamEvaluation]
    /// Teams that could not answer, so that one of them does not blank the others.
    public let unavailable: [TeamUnavailable]
}

/// A team the node could not get numbers for. The reason is plain text, with no secret and no address in it.
public struct TeamUnavailable: Decodable, Equatable, Sendable {
    public let team: String
    @Shown public private(set) var reason: String
}

public struct TeamEvaluation: Decodable, Equatable, Sendable {
    public let team: String
    /// The period's first and last calendar day in the time zone of the team's node, written like `DayEvaluation.date`.
    /// Every day in `byDay` lies inside it, so a team is drawn against its own period.
    public let from: String
    public let until: String
    public let summary: EvaluationSummary
    /// Oldest day first; days with nothing in them may be left out.
    public let byDay: [DayEvaluation]
    /// What each participant did over the period. There only for the team's owner reading on their own node, and only when the
    /// team's runtime reports it: without it nothing about participants is known, which is not the same as none.
    public let participants: [ParticipantEvaluation]?
    /// The pull requests merged in the period that no participant is named the author of, so that the participants' `merged` and
    /// this add up to the summary's. There with `participants`.
    public let unattributedMerged: Int?
}

/// One participant's numbers over the period, from the team runtime's own record and the pull requests GitHub says merged.
public struct ParticipantEvaluation: Decodable, Equatable, Sendable {
    /// The participant's name in the team's runtime.
    public let name: String
    /// Seconds in the period at least one of the participant's sessions was working, was waiting for a human's decision (a lower
    /// bound: a runtime sees only some of the waits), or was alive and neither. Overlapping sessions count once.
    public let busySeconds: Double
    public let waitingSeconds: Double
    public let idleSeconds: Double
    public let costUsd: Double
    public let tokens: TokenCounts
    /// The pull requests merged in the period (`merged` and `reverts` of the summary together) whose author is this participant.
    public let merged: Int
    /// `costUsd` over `merged`; left out when the participant merged nothing.
    public let costPerMergedPrUsd: Double?

    private enum Keys: String, CodingKey {
        case name, busySeconds, waitingSeconds, idleSeconds, costUsd, tokens, merged, costPerMergedPrUsd
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        name = NodeLabel.shown(try container.decode(String.self, forKey: .name))
        busySeconds = try container.amount(.busySeconds)
        waitingSeconds = try container.amount(.waitingSeconds)
        idleSeconds = try container.amount(.idleSeconds)
        costUsd = try container.amount(.costUsd)
        tokens = try container.decode(TokenCounts.self, forKey: .tokens)
        merged = Int(try container.amount(.merged))
        costPerMergedPrUsd = try container.optionalAmount(.costPerMergedPrUsd)
    }
}

/// Merged pull requests by the Conventional Commits type of their titles; anything else is `other`.
/// A pull request that reverts another is counted in `reverts`, not here.
public struct MergedCounts: Decodable, Equatable, Sendable {
    /// The types in the order they are listed and stacked.
    public enum Kind: String, CaseIterable, Sendable { case feat, fix, refactor, docs, test, chore, other }

    public let feat: Int
    public let fix: Int
    public let refactor: Int
    public let docs: Int
    public let test: Int
    public let chore: Int
    public let other: Int

    public var total: Int { Kind.allCases.reduce(0) { $0 + self[$1] } }

    public subscript(kind: Kind) -> Int {
        switch kind {
        case .feat: feat
        case .fix: fix
        case .refactor: refactor
        case .docs: docs
        case .test: test
        case .chore: chore
        case .other: other
        }
    }

    public init(feat: Int = 0, fix: Int = 0, refactor: Int = 0, docs: Int = 0, test: Int = 0, chore: Int = 0, other: Int = 0) {
        self.feat = feat
        self.fix = fix
        self.refactor = refactor
        self.docs = docs
        self.test = test
        self.chore = chore
        self.other = other
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: AnyKey.self)
        func count(_ kind: Kind) throws -> Int { try container.count(kind.rawValue) }
        feat = try count(.feat)
        fix = try count(.fix)
        refactor = try count(.refactor)
        docs = try count(.docs)
        test = try count(.test)
        chore = try count(.chore)
        other = try count(.other)
    }
}

/// The numbers for the whole period. A median or p90 is left out when there are no samples; the p90 is the
/// nearest rank, the smallest lead time that at least 9 in 10 of the period's pull requests do not exceed.
public struct EvaluationSummary: Decodable, Equatable, Sendable {
    public let merged: MergedCounts
    public let prLeadMedianSeconds: Double?
    public let prLeadP90Seconds: Double?
    public let issueLeadMedianSeconds: Double?
    public let issueLeadP90Seconds: Double?
    public let mainRedSeconds: Double
    /// The longest single span main was red in the period.
    public let longestRedSeconds: Double
    public let reverts: Int
    /// The period's cost divided by the pull requests merged in it (`merged` and `reverts`); left out when nothing merged.
    public let costPerMergedPrUsd: Double?

    private enum Keys: String, CodingKey {
        case merged, prLeadMedianSeconds, prLeadP90Seconds, issueLeadMedianSeconds, issueLeadP90Seconds
        case mainRedSeconds, longestRedSeconds, reverts, costPerMergedPrUsd
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        merged = try container.decodeIfPresent(MergedCounts.self, forKey: .merged) ?? MergedCounts()
        prLeadMedianSeconds = try container.optionalAmount(.prLeadMedianSeconds)
        prLeadP90Seconds = try container.optionalAmount(.prLeadP90Seconds)
        issueLeadMedianSeconds = try container.optionalAmount(.issueLeadMedianSeconds)
        issueLeadP90Seconds = try container.optionalAmount(.issueLeadP90Seconds)
        mainRedSeconds = try container.amount(.mainRedSeconds)
        longestRedSeconds = try container.amount(.longestRedSeconds)
        reverts = Int(try container.amount(.reverts))
        costPerMergedPrUsd = try container.optionalAmount(.costPerMergedPrUsd)
    }
}

/// One calendar day in the node's time zone.
public struct DayEvaluation: Decodable, Equatable, Sendable {
    public let date: String
    public let merged: MergedCounts
    /// Pull requests merged that day that revert another.
    public let reverts: Int
    /// Seconds main's CI was red that day; a span across midnight is split between the days.
    public let mainRedSeconds: Double
    /// Estimated cost of that day's tokens, API-priced.
    public let costUsd: Double
    /// Pull request opened to merged, one for each pull request merged that day.
    public let prLeadSeconds: [Double]
    /// Issue started to merged, one for each merged pull request that has an issue.
    public let issueLeadSeconds: [Double]

    private enum Keys: String, CodingKey {
        case date, merged, reverts, mainRedSeconds, costUsd, prLeadSeconds, issueLeadSeconds
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        date = try container.decode(String.self, forKey: .date)
        merged = try container.decodeIfPresent(MergedCounts.self, forKey: .merged) ?? MergedCounts()
        reverts = Int(try container.amount(.reverts))
        mainRedSeconds = try container.amount(.mainRedSeconds)
        costUsd = try container.amount(.costUsd)
        prLeadSeconds = try container.amounts(.prLeadSeconds)
        issueLeadSeconds = try container.amounts(.issueLeadSeconds)
    }

    public init(date: String, merged: MergedCounts = MergedCounts(), reverts: Int = 0, mainRedSeconds: Double = 0, costUsd: Double = 0, prLeadSeconds: [Double] = [], issueLeadSeconds: [Double] = []) {
        self.date = date
        self.merged = merged
        self.reverts = reverts
        self.mainRedSeconds = mainRedSeconds
        self.costUsd = costUsd
        self.prLeadSeconds = prLeadSeconds
        self.issueLeadSeconds = issueLeadSeconds
    }
}

// MARK: Reading a number the way the web client does

struct AnyKey: CodingKey {
    let stringValue: String
    let intValue: Int? = nil
    init(stringValue: String) { self.stringValue = stringValue }
    init?(intValue: Int) { nil }
}

extension KeyedDecodingContainer {
    /// A non-negative number; a missing one is 0. A negative one refuses the answer.
    func amount(_ key: Key) throws -> Double {
        try optionalAmount(key) ?? 0
    }

    func optionalAmount(_ key: Key) throws -> Double? {
        guard let value = try decodeIfPresent(Double.self, forKey: key) else { return nil }
        guard value.isFinite, value >= 0 else {
            throw DecodingError.dataCorruptedError(forKey: key, in: self, debugDescription: "not a non-negative number")
        }
        return value
    }

    /// A list of non-negative numbers; a missing list is empty.
    func amounts(_ key: Key) throws -> [Double] {
        let values = try decodeIfPresent([Double].self, forKey: key) ?? []
        guard values.allSatisfy({ $0.isFinite && $0 >= 0 }) else {
            throw DecodingError.dataCorruptedError(forKey: key, in: self, debugDescription: "not non-negative numbers")
        }
        return values
    }
}

extension KeyedDecodingContainer where Key == AnyKey {
    /// A whole count of one type; a missing one is 0.
    func count(_ name: String) throws -> Int {
        let key = AnyKey(stringValue: name)
        guard let value = try optionalAmount(key) else { return 0 }
        guard let whole = Int(exactly: value) else {
            throw DecodingError.dataCorruptedError(forKey: key, in: self, debugDescription: "not a whole number")
        }
        return whole
    }
}
