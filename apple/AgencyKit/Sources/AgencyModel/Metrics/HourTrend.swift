import AgencyAPI
import Foundation

/// What an hour says: nothing (no call), or its 95th percentile as the upper bound of its bucket, or slower than the last bucket.
public struct HourPoint: Equatable, Identifiable, Sendable {
    /// The start of the hour.
    public let at: Date
    /// The calls in the hour, over every route of the chart.
    public let count: Int
    /// Nothing when there was no call: a gap, not a 0.
    public let bound: Bound?
    /// Whether the bound is the chart's own p95, read from the hour's buckets added together; otherwise it is the slowest route's, which the
    /// chart's own is at most.
    public let isExact: Bool

    public var id: Date { at }
}

/// One small chart of the hourly trend: a route group (or the in-node reads, against the budget) over the hours of the range.
public struct HourChart: Equatable, Identifiable, Sendable {
    public let id: String
    public let title: String
    /// The budget, drawn as a line; only on the chart of the in-node reads, which it applies to.
    public let budgetMs: Int?
    public let points: [HourPoint]
    /// Whether every hour with calls is the chart's own p95. A chart is decided as a whole, so that one line never mixes two meanings.
    public let isExact: Bool

    /// One line of the table of hours: the hour, its calls and its p95, as written.
    public struct Row: Equatable, Identifiable, Sendable {
        public let id: Date
        public let hour: String
        public let calls: String
        public let p95: String
    }

    /// The hours with calls, newest first, as the table says them: what is said to anyone who cannot see the picture.
    public var rows: [Row] {
        points.reversed().compactMap { point in
            point.bound.map { Row(id: point.at, hour: JaFormat.dateTime(point.at), calls: JaFormat.integer(point.count), p95: Ja.NodeSpeed.bound($0)) }
        }
    }

    /// What the picture says in one sentence, for VoiceOver; nothing for a chart with no hour of calls.
    public var summary: String? {
        let withCalls = points.filter { $0.bound != nil }
        guard let latest = withCalls.last?.bound else { return nil }
        let hours = String(withCalls.count)
        let words = Ja.NodeSpeed.Trend.self
        return isExact ? words.summary(title, hours, Ja.NodeSpeed.bound(latest)) : words.summaryApproximate(title, hours, Ja.NodeSpeed.bound(latest))
    }
}

/// The hour-by-hour trend of the node's own response time, worked out of the series' `byHour`. With the buckets of every route's hour the
/// chart's own p95 is read from them added together, as the period's is. Without them the p95 of different routes cannot be put together,
/// and what is drawn is the slowest route's p95 of the hour, which the chart's own is at most. Either way it is an upper bound of a bucket,
/// and said "at most" like every percentile.
public enum HourTrend {
    private static let hourSeconds: TimeInterval = 3600

    /// The longest range whose hours the node gives (7 days).
    public static let maxHours = 24 * 7

    /// The periods, in days, that are asked for their hours: a longer range comes with totals only.
    public static func hasHours(days: Int) -> Bool { days <= maxHours / 24 }

    /// The start of each hour of `from` to `until`, oldest first; none when the range is not hours of at most 7 days.
    public static func hours(from: Instant, until: Instant) -> [Date] {
        let start = from.date
        let end = until.date
        guard end > start else { return [] }
        let count = Int(((end.timeIntervalSince(start)) / hourSeconds).rounded())
        guard count > 0, count <= maxHours else { return [] }
        return (0..<count).map { start.addingTimeInterval(Double($0) * hourSeconds) }
    }

    private struct HourSums {
        var count = 0
        /// The slowest route's bound; `.beyond` when a route with calls gave no p95.
        var slowest: Bound?
        /// The hour's buckets added up, while every route that had calls in it gave its own.
        var buckets: [Int]?
        var isFirst = true
    }

    private static func point(_ at: Date, _ found: HourSums?, useBuckets: Bool) -> HourPoint {
        guard let found, found.count > 0 else { return HourPoint(at: at, count: 0, bound: nil, isExact: true) }
        if useBuckets, let buckets = found.buckets, let exact = Latency.percentile(of: buckets, percent: 95) {
            return HourPoint(at: at, count: found.count, bound: exact, isExact: true)
        }
        guard let slowest = found.slowest else { return HourPoint(at: at, count: 0, bound: nil, isExact: true) }
        return HourPoint(at: at, count: found.count, bound: slowest, isExact: false)
    }

    private static func slower(_ known: Bound?, than entry: HourSeries, isFirst: Bool) -> Bound {
        guard let p95 = entry.p95Ms else { return .beyond }
        guard !isFirst, let known else { return .atMost(p95) }
        switch known {
        case .beyond: return .beyond
        case .atMost(let ms): return .atMost(max(ms, p95))
        }
    }

    private static func added(_ sum: [Int]?, _ more: [Int]) -> [Int] {
        guard let sum else { return more }
        return zip(sum, more).map(+)
    }

    /// One point for each of the hours, a gap where no call was made. With the buckets of every route's hour the point is the chart's own
    /// p95; without them (a node that does not give them, or gave them to some routes only) it is the slowest route's p95, and a route with
    /// calls and no p95 makes the hour "beyond".
    public static func points(of series: [LatencySeries], hours: [Date], useBuckets: Bool = true) -> [HourPoint] {
        var byHour: [Date: HourSums] = [:]
        for one in series {
            for entry in one.byHour where entry.count > 0 {
                var sums = byHour[entry.hour.date] ?? HourSums()
                sums.slowest = slower(sums.slowest, than: entry, isFirst: sums.isFirst)
                if let buckets = entry.buckets, sums.isFirst || sums.buckets != nil {
                    sums.buckets = added(sums.buckets, buckets)
                } else {
                    sums.buckets = nil
                }
                sums.count += entry.count
                sums.isFirst = false
                byHour[entry.hour.date] = sums
            }
        }
        return hours.map { point($0, byHour[$0], useBuckets: useBuckets) }
    }

    /// The points of one chart, decided for the whole chart: the chart's own p95 when every hour with calls has it, and otherwise the slowest
    /// route's p95 for every hour.
    public static func chartPoints(of series: [LatencySeries], hours: [Date]) -> (points: [HourPoint], isExact: Bool) {
        let exact = points(of: series, hours: hours)
        if exact.allSatisfy({ $0.bound == nil || $0.isExact }) { return (exact, true) }
        return (points(of: series, hours: hours, useBuckets: false), false)
    }

    /// The charts of the trend: the in-node reads against the budget (when the node names one), then each route group, then each group of
    /// calls, in the order of the tables above them (busiest first). A chart with no hour of calls is not drawn.
    public static func charts(of series: [LatencySeries], budgetMs: Int?, hours: [Date]) -> [HourChart] {
        var charts: [HourChart] = []
        if let budgetMs {
            let reads = series.filter { $0.kind == .serve && $0.isInNodeRead }
            charts.append(chart("in-node-reads", Ja.NodeSpeed.Trend.inNodeReads, reads, budgetMs: budgetMs, hours: hours))
        }
        for group in Latency.groups(of: series, kind: .serve) {
            let members = series.filter { $0.kind == .serve && $0.group == group.id }
            charts.append(chart("serve-\(group.id)", Ja.NodeSpeed.Groups.name(group.id), members, budgetMs: nil, hours: hours))
        }
        for group in Latency.groups(of: series, kind: .call) {
            let members = series.filter { $0.kind == .call && $0.group == group.id }
            charts.append(chart("call-\(group.id)", Ja.NodeSpeed.Trend.callGroup(Ja.NodeSpeed.Groups.name(group.id)), members, budgetMs: nil, hours: hours))
        }
        return charts.filter { chart in chart.points.contains { $0.bound != nil } }
    }

    private static func chart(_ id: String, _ title: String, _ members: [LatencySeries], budgetMs: Int?, hours: [Date]) -> HourChart {
        let trend = chartPoints(of: members, hours: hours)
        return HourChart(id: id, title: title, budgetMs: budgetMs, points: trend.points, isExact: trend.isExact)
    }
}

/// The scale of an hourly chart: milliseconds only, on a log scale from the first bound to the last (a p95 runs from 1 ms to 30 s).
public enum HourScale {
    public static let minMs = 0.5
    public static let maxMs = 30_000.0
    public static let ticksMs = [1, 10, 100, 1000, 30_000]

    /// Where a value stands, from 0 (bottom) to 1 (top); what is slower than the last bound is at the top.
    public static func share(ms: Double) -> Double {
        log10(min(max(ms, minMs), maxMs) / minMs) / log10(maxMs / minMs)
    }

    /// Where a point stands; nothing for an hour with no call, and the top for one slower than the last bound.
    public static func share(of bound: Bound?) -> Double? {
        switch bound {
        case nil: nil
        case .beyond?: 1
        case .atMost(let ms)?: share(ms: Double(ms))
        }
    }
}
