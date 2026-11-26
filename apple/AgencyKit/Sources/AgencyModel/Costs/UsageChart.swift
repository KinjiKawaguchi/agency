import AgencyAPI
import Foundation

/// What the costs screen draws from one team's usage report, ported from `web/src/costs/usage.ts`.
public enum UsageChart {
    /// Colors available for series. A ninth participant is folded into "other", never given a new hue.
    public static let maxSeries = 8

    public struct Series: Equatable, Sendable, Identifiable {
        /// The team runtime's participant name, or `nil` for the folded "other" series.
        public let person: String?
        public let label: String
        /// The color slot, 1 to 8, fixed per participant so a change of period never repaints the others; `nil` for "other".
        public let slot: Int?
        public var id: String { person ?? "\u{0}other" }
    }

    public struct Column: Equatable, Sendable, Identifiable {
        public let date: String
        public let total: Double
        /// Cost per series, in series order.
        public let values: [Double]
        public var id: String { date }
    }

    public struct Totals: Equatable, Sendable {
        public let costUsd: Double
        public let perDay: Double
        public let output: Int
        public let inputSide: Int
        /// Share of the input side that was read from the cache, 0 to 1.
        public let cacheShare: Double
    }

    /// Series follow the team's own participant order, so a participant keeps their color as the period changes.
    /// Participants beyond the eighth, and any the team no longer lists, are folded into one "other" series.
    public static func series(for report: TokenUsage, teamOrder: [String], labelOf: (String) -> String, otherLabel: String) -> [Series] {
        let used = Set(report.byPerson.map(\.person))
        let ordered = teamOrder.filter { used.contains($0) } + unique(report.byPerson.map(\.person)).filter { !teamOrder.contains($0) }
        let shown = ordered.prefix(maxSeries).enumerated().map { Series(person: $1, label: labelOf($1), slot: $0 + 1) }
        return ordered.count > maxSeries ? shown + [Series(person: nil, label: otherLabel, slot: nil)] : shown
    }

    public static func columns(for report: TokenUsage, series: [Series]) -> [Column] {
        let named = Set(series.compactMap(\.person))
        return report.byDay.map { day in
            let values = series.map { one -> Double in
                guard let person = one.person else { return day.costUsd.filter { !named.contains($0.key) }.values.reduce(0, +) }
                return day.costUsd[person] ?? 0
            }
            return Column(date: day.date, total: values.reduce(0, +), values: values)
        }
    }

    public static func totals(of report: TokenUsage) -> Totals {
        let tokens = report.byPerson.map(\.tokens)
        let inputSide = tokens.reduce(0) { $0 + $1.input + $1.cacheRead + $1.cacheWrite }
        let cost = report.byPerson.reduce(0) { $0 + $1.costUsd }
        return Totals(
            costUsd: cost, perDay: cost / Double(max(report.byDay.count, 1)), output: tokens.reduce(0) { $0 + $1.output },
            inputSide: inputSide, cacheShare: inputSide == 0 ? 0 : Double(tokens.reduce(0) { $0 + $1.cacheRead }) / Double(inputSide)
        )
    }

    private static func unique(_ names: [String]) -> [String] {
        var seen = Set<String>()
        return names.filter { seen.insert($0).inserted }
    }
}
