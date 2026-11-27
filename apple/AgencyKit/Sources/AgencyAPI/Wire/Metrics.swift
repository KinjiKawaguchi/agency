import AgencyProtocol
import Foundation

/// `GET /api/metrics?days=<1 to 30>`: the node's own measurements over a range (`docs/protocol.md`, "Metrics"). Owner only: anyone
/// else is told 403, and a node that does not measure 404. An answer with no series is a node that measured and has nothing yet.
public struct Metrics: Decodable, Equatable, Sendable {
    public let from: Instant
    public let until: Instant
    public let generatedAt: Instant
    /// What an in-node read is held to; absent when the node says none.
    public let budget: Budget?
    /// The response times the node's operator set for route groups, with what the node says of each; absent when none is set.
    public let objectives: [Objective]
    public let series: [LatencySeries]

    public init(range: (from: Instant, until: Instant), generatedAt: Instant, budget: Budget?, objectives: [Objective], series: [LatencySeries]) {
        from = range.from
        until = range.until
        self.generatedAt = generatedAt
        self.budget = budget
        self.objectives = objectives
        self.series = series
    }

    private enum Keys: String, CodingKey { case from, until, generatedAt, budget, objectives, series }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        from = try container.decode(Instant.self, forKey: .from)
        until = try container.decode(Instant.self, forKey: .until)
        generatedAt = try container.decode(Instant.self, forKey: .generatedAt)
        budget = try container.decodeIfPresent(Budget.self, forKey: .budget)
        objectives = try container.decodeIfPresent([Objective].self, forKey: .objectives) ?? []
        series = try container.decode([LatencySeries].self, forKey: .series)
    }
}

public struct Budget: Decodable, Equatable, Sendable {
    /// The most the 95th percentile of an in-node read may be, in milliseconds.
    public let inNodeReadP95Ms: Int

    public init(inNodeReadP95Ms: Int) { self.inNodeReadP95Ms = inNodeReadP95Ms }
}

/// A response time the node's operator set for a route group, and whether it held over the days asked. The node compares; the
/// app only says what it says.
public struct Objective: Decodable, Equatable, Sendable {
    /// `undetermined`: the objective falls inside the bucket the p95 is known by, so the measurement cannot say (the same three-outcome
    /// rule as the budget); `unknown`: no call in the range, or a state this app does not know.
    public enum State: String, OpenWireEnum { case met, notMet, undetermined, unknown }

    public let group: String
    public let p95Ms: Int
    public let state: State
    /// What the node measured (an upper bound), when it could.
    public let measuredP95Ms: Int?
}

/// One kind, name and status (and peer or client) of what the node measured, as a histogram.
public struct LatencySeries: Decodable, Equatable, Sendable {
    public enum Kind: String, OpenWireEnum { case serve, call, client, unknown }

    /// The fixed upper bounds, in milliseconds, of the first 14 buckets; the 15th holds what is slower than the last.
    public static let bucketBoundsMs = [1, 2, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10_000, 30_000]
    public static let bucketCount = bucketBoundsMs.count + 1

    public let kind: Kind
    /// A route template (`GET /api/orgs/{org}/teams/{team}/usage`) or a fixed phrase (`runtime view`), never a path with values.
    public let name: String
    public let status: String
    /// The route group the node says, or the first word of a call.
    public let group: String
    /// For a call to a peer: the host of its origin.
    public let peer: String?
    public let client: String?
    /// Whether the budget applies: the node answers the route from its own state alone.
    public let isInNodeRead: Bool
    public let count: Int
    public let errors: Int
    public let sumMs: Int
    /// `bucketCount` counts that add up to `count`.
    public let buckets: [Int]
    public let p50Ms: Int?
    public let p95Ms: Int?
    public let p99Ms: Int?
    /// The same series by hour of UTC, oldest first; an hour with no call may be missing. Empty from a node that gives no hours.
    public let byHour: [HourSeries]

    private enum Keys: String, CodingKey {
        case kind, name, status, group, peer, client, isInNodeRead, count, errors, sumMs, buckets, p50Ms, p95Ms, p99Ms, byHour
    }

    /// An answer whose buckets deny its count, or that has more errors than calls, is refused whole: a screen must never show a
    /// percentile that its own buckets deny.
    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        kind = try container.decode(Kind.self, forKey: .kind)
        name = try container.decode(String.self, forKey: .name)
        status = try container.decode(String.self, forKey: .status)
        group = try container.decode(String.self, forKey: .group)
        peer = try container.decodeIfPresent(String.self, forKey: .peer)
        client = try container.decodeIfPresent(String.self, forKey: .client)
        isInNodeRead = try container.decode(Bool.self, forKey: .isInNodeRead) && kind == .serve
        count = try container.decode(Int.self, forKey: .count)
        errors = try container.decode(Int.self, forKey: .errors)
        sumMs = try container.decode(Int.self, forKey: .sumMs)
        buckets = try container.decode([Int].self, forKey: .buckets)
        p50Ms = try container.decodeIfPresent(Int.self, forKey: .p50Ms)
        p95Ms = try container.decodeIfPresent(Int.self, forKey: .p95Ms)
        p99Ms = try container.decodeIfPresent(Int.self, forKey: .p99Ms)
        byHour = try container.decodeIfPresent([HourSeries].self, forKey: .byHour) ?? []
        let amounts = [count, errors, sumMs] + buckets + [p50Ms, p95Ms, p99Ms].compactMap { $0 }
        guard amounts.allSatisfy({ $0 >= 0 }), buckets.count == Self.bucketCount, buckets.reduce(0, +) == count, errors <= count else {
            throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "A series whose numbers deny each other: \(name)"))
        }
    }

    public init(
        kind: Kind, name: String, status: String = "2xx", group: String, peer: String? = nil, isInNodeRead: Bool = false,
        count: Int, errors: Int = 0, buckets: [Int]
    ) {
        self.kind = kind
        self.name = name
        self.status = status
        self.group = group
        self.peer = peer
        client = nil
        self.isInNodeRead = isInNodeRead
        self.count = count
        self.errors = errors
        sumMs = 0
        self.buckets = buckets
        p50Ms = nil
        p95Ms = nil
        p99Ms = nil
        byHour = []
    }
}

/// One hour of a series (`HourSeries`). Its `buckets` come only in an answer asked for them (`hourBuckets=1`), so that the hours of
/// several series can be added and the percentile of their whole read from the sum.
public struct HourSeries: Decodable, Equatable, Sendable {
    public let hour: Instant
    public let count: Int
    public let errors: Int
    /// The upper bound of the bucket of the hour's 95th percentile; left out when it is beyond the last bound.
    public let p95Ms: Int?
    /// `LatencySeries.bucketCount` counts that add up to `count`.
    public let buckets: [Int]?

    private enum Keys: String, CodingKey { case hour, count, errors, p95Ms, buckets }

    /// An hour whose buckets deny its count would give a percentile that its own numbers deny: the whole answer is refused.
    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        hour = try container.decode(Instant.self, forKey: .hour)
        count = try container.decode(Int.self, forKey: .count)
        errors = try container.decode(Int.self, forKey: .errors)
        p95Ms = try container.decodeIfPresent(Int.self, forKey: .p95Ms)
        buckets = try container.decodeIfPresent([Int].self, forKey: .buckets)
        let amounts = [count, errors] + (buckets ?? []) + [p95Ms].compactMap { $0 }
        let isBucketsRight = buckets.map { $0.count == LatencySeries.bucketCount && $0.reduce(0, +) == count } ?? true
        guard amounts.allSatisfy({ $0 >= 0 }), isBucketsRight else {
            throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "An hour whose numbers deny each other"))
        }
    }

    public init(hour: Instant, count: Int, errors: Int = 0, p95Ms: Int? = nil, buckets: [Int]? = nil) {
        self.hour = hour
        self.count = count
        self.errors = errors
        self.p95Ms = p95Ms
        self.buckets = buckets
    }
}
