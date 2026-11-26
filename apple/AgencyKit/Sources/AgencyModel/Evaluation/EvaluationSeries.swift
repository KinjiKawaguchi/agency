import AgencyAPI
import Foundation

/// The days and the numbers over them that the evaluation charts draw, ported from `web/src/evaluation/series.ts`.
public enum EvaluationSeries {
    public static let secondsPerMinute = 60.0
    public static let secondsPerHour = 3_600.0
    public static let secondsPerDay = 86_400.0

    // MARK: Days

    private static var utc: Calendar {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "UTC") ?? .gmt
        return calendar
    }

    /// A calendar day written `YYYY-MM-DD`.
    static func date(_ day: String) -> Date? {
        let parts = day.split(separator: "-").compactMap { Int($0) }
        guard parts.count == 3 else { return nil }
        return utc.date(from: DateComponents(year: parts[0], month: parts[1], day: parts[2]))
    }

    static func day(_ date: Date) -> String {
        let parts = utc.dateComponents([.year, .month, .day], from: date)
        return String(format: "%04d-%02d-%02d", parts.year ?? 0, parts.month ?? 0, parts.day ?? 0)
    }

    /// How many calendar days lie from `from` to `until`, both included; `nil` when either is not a day or `until` is before `from`.
    public static func dayCount(from: String, until: String) -> Int? {
        guard let start = date(from), let end = date(until), end >= start else { return nil }
        return (utc.dateComponents([.day], from: start, to: end).day ?? 0) + 1
    }

    /// The `days` calendar days that end at `last`, oldest first. Nothing when `last` is not a day.
    public static func daysEnding(_ last: String, days: Int) -> [String] {
        guard let end = date(last) else { return [] }
        return (0..<max(days, 0)).compactMap { offset in
            utc.date(byAdding: .day, value: offset - (days - 1), to: end).map(day)
        }
    }

    /// The days of the period, oldest first and none missing: the node may leave out a day with nothing in it.
    public static func fillDays(_ byDay: [DayEvaluation], days: Int, last: String) -> [DayEvaluation] {
        let known = Dictionary(byDay.map { ($0.date, $0) }, uniquingKeysWith: { first, _ in first })
        return daysEnding(last, days: days).map { known[$0] ?? DayEvaluation(date: $0) }
    }

    // MARK: Numbers over the days

    public static func median(_ values: [Double]) -> Double? {
        guard !values.isEmpty else { return nil }
        let sorted = values.sorted()
        let middle = sorted.count / 2
        return sorted.count % 2 == 1 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2
    }

    /// The days of the trailing window that ends at each day: the day itself and the ones before it.
    private static func trailing(_ rows: [DayEvaluation], window: Int) -> [[DayEvaluation]] {
        rows.indices.map { Array(rows[max(0, $0 - window + 1)...$0]) }
    }

    /// The median of every lead time in the trailing window, per day; `nil` while the window has none.
    public static func rollingMedian(_ rows: [DayEvaluation], window: Int, of pick: (DayEvaluation) -> [Double]) -> [Double?] {
        trailing(rows, window: window).map { median($0.flatMap(pick)) }
    }

    /// The pull requests merged in a day: those counted by type, and the ones that revert another.
    public static func pullRequestsMerged(_ day: DayEvaluation) -> Int { day.merged.total + day.reverts }

    /// Cost per merged pull request over the trailing window; `nil` while nothing was merged in it.
    public static func rollingCostPerPullRequest(_ rows: [DayEvaluation], window: Int) -> [Double?] {
        trailing(rows, window: window).map { span in
            let merged = span.reduce(0) { $0 + pullRequestsMerged($1) }
            return merged == 0 ? nil : span.reduce(0) { $0 + $1.costUsd } / Double(merged)
        }
    }

    // MARK: Axes

    /// The smallest of 1, 2, 5 times a power of ten that is at least `value`; 1 for nothing, so an empty chart still has an axis.
    public static func niceMax(_ value: Double) -> Double {
        guard value > 0 else { return 1 }
        let magnitude = pow(10, floor(log10(value)))
        let step = [1.0, 2, 5, 10].first { $0 * magnitude >= value } ?? 10
        return step * magnitude
    }

    /// An axis maximum for counts: whole numbers all the way, so that the middle tick is whole as well.
    public static func countMax(_ value: Double) -> Double {
        max(2, 2 * ceil(niceMax(value) / 2))
    }
}
