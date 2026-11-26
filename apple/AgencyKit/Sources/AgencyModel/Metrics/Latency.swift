import AgencyAPI
import Foundation

/// A percentile as the node gives it: the upper bound of the bucket the call fell in, so it is "at most" that, never exact; or
/// beyond the last bucket, which has no bound (slower than 30 seconds).
public enum Bound: Equatable, Sendable {
    case atMost(Int)
    case beyond
}

/// What a screen works out of the node's series: the percentile of several series put together, the rows by route group and by
/// route, and whether in-node reads keep the budget. A series' own percentiles are never averaged: buckets are added and the
/// percentile is read from the sum, as the node reads it from one.
public enum Latency {
    /// Nearest rank, as the node reads it: the `ceil(n × percent / 100)`-th duration, given as its bucket's upper bound. Nothing
    /// when there is no duration. Integers all the way, so that no rounding can move the rank.
    public static func percentile(of buckets: [Int], percent: Int) -> Bound? {
        let total = buckets.reduce(0, +)
        guard total > 0 else { return nil }
        let rank = max(1, (total * percent + 99) / 100)
        var seen = 0
        for (index, inBucket) in buckets.enumerated() {
            seen += inBucket
            guard seen >= rank else { continue }
            return index < LatencySeries.bucketBoundsMs.count ? .atMost(LatencySeries.bucketBoundsMs[index]) : .beyond
        }
        return .beyond
    }

    public struct Row: Equatable, Sendable, Identifiable {
        /// What the row is named: a route group, a route template, or a call (with its peer).
        public let key: String
        public let count: Int
        public let errors: Int
        /// Nothing when the row has no duration: a cell with nothing to say says nothing, not a dash.
        public let p50: Bound?
        public let p95: Bound?
        public let p99: Bound?
        public var id: String { key }
    }

    public struct Group: Equatable, Sendable, Identifiable {
        public let row: Row
        public let routes: [Row]
        public var id: String { row.key }
    }

    static func row(_ key: String, _ series: [LatencySeries]) -> Row {
        var buckets = Array(repeating: 0, count: LatencySeries.bucketCount)
        for one in series { for (index, inBucket) in one.buckets.enumerated() { buckets[index] += inBucket } }
        return Row(
            key: key, count: series.reduce(0) { $0 + $1.count }, errors: series.reduce(0) { $0 + $1.errors },
            p50: percentile(of: buckets, percent: 50), p95: percentile(of: buckets, percent: 95), p99: percentile(of: buckets, percent: 99)
        )
    }

    /// A call is told apart by the peer it went to; a route has no peer.
    private static func routeKey(_ one: LatencySeries) -> String { one.peer.map { "\(one.name) (\($0))" } ?? one.name }

    private static func grouped(_ series: [LatencySeries], by key: (LatencySeries) -> String) -> [(key: String, series: [LatencySeries])] {
        var order: [String] = []
        var found: [String: [LatencySeries]] = [:]
        for one in series {
            let name = key(one)
            if found[name] == nil { order.append(name) }
            found[name, default: []].append(one)
        }
        return order.map { (key: $0, series: found[$0] ?? []) }
    }

    private static func busiestFirst(_ rows: [Row]) -> [Row] {
        rows.sorted { $0.count != $1.count ? $0.count > $1.count : $0.key < $1.key }
    }

    /// The series of one kind by route group, each group with its routes (statuses and clients put together), busiest first. A
    /// series with no call is not a row.
    public static func groups(of series: [LatencySeries], kind: LatencySeries.Kind) -> [Group] {
        let measured = series.filter { $0.kind == kind && $0.count > 0 }
        let groups = grouped(measured, by: \.group).map { entry in
            Group(row: row(entry.key, entry.series), routes: busiestFirst(grouped(entry.series, by: routeKey).map { row($0.key, $0.series) }))
        }
        return groups.sorted { $0.row.count != $1.row.count ? $0.row.count > $1.row.count : $0.row.key < $1.row.key }
    }

    public enum Verdict: Equatable, Sendable { case within, over }

    /// The budget is a test of the p95 against a bound: `p95 <= budget`. A p95 beyond the last bucket is over it.
    public static func verdict(of p95: Bound?, budgetMs: Int) -> Verdict? {
        guard let p95 else { return nil }
        if case .atMost(let ms) = p95, ms <= budgetMs { return .within }
        return .over
    }

    private static func inNodeReads(_ series: [LatencySeries]) -> [LatencySeries] {
        series.filter { $0.kind == .serve && $0.isInNodeRead && $0.count > 0 }
    }

    /// Every in-node read put together, for the budget; nothing when none was measured.
    public static func inNodeReadsRow(_ series: [LatencySeries]) -> Row? {
        let reads = inNodeReads(series)
        return reads.isEmpty ? nil : row("in-node reads", reads)
    }

    /// Each in-node read route's own row that breaks the budget, busiest first.
    public static func readsOverBudget(_ series: [LatencySeries], budgetMs: Int) -> [Row] {
        busiestFirst(grouped(inNodeReads(series), by: routeKey).map { row($0.key, $0.series) }).filter { verdict(of: $0.p95, budgetMs: budgetMs) == .over }
    }
}
