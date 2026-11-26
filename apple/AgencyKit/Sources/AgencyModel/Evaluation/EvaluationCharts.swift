import AgencyAPI
import Foundation

/// One series of a chart: a pull request type, or a line. The slot is its color, fixed for the series.
public struct ChartSeries: Equatable, Sendable, Identifiable {
    public let key: String
    public let label: String
    /// The color slot, 1 to 8; `nil` for the grey of "other".
    public let slot: Int?
    public var id: String { key }
}

/// One day of a chart, with everything the screen says of it.
public struct ChartDay: Equatable, Sendable, Identifiable {
    public let date: String
    public let shortDate: String
    /// One value for each series; `nil` where there is nothing to say, which breaks a line.
    public let values: [Double?]
    /// What each value is called when the day is opened.
    public let valueTexts: [String]
    /// The sum of a stack, when the day says it.
    public let totalText: String?
    /// How the day reads aloud.
    public let spoken: String
    public var id: String { date }

    public var total: Double { values.reduce(0) { $0 + ($1 ?? 0) } }
}

/// A chart of the evaluation: columns by day, or lines over the days. The axis is 0 at the bottom and `max` at the top.
public struct EvaluationChart: Equatable, Sendable, Identifiable {
    public enum Kind: Sendable { case columns, lines }

    public let title: String
    public let hint: String?
    public let kind: Kind
    public let series: [ChartSeries]
    public let days: [ChartDay]
    public let max: Double
    /// The ticks, top to bottom.
    public let axis: [String]
    /// The first, middle and last day, written under the chart.
    public let dateLabels: [String]
    /// There is nothing to draw: a line chart with no value at all.
    public let hasNoData: Bool
    public var id: String { title }
}

enum ChartBuilder {
    /// The days written under a chart: the first, the middle and the last.
    static func dateLabels(_ dates: [String]) -> [String] {
        guard let first = dates.first, let last = dates.last else { return [] }
        return [first, dates[(dates.count - 1) / 2], last].map(JaFormat.shortDate)
    }

    static func columns(
        title: String, hint: String? = nil, series: [ChartSeries], rows: [DayEvaluation], max: Double,
        axis: [String], values: (DayEvaluation) -> [Double], text: (Double) -> String, hasTotal: Bool, spoken: (String, Double) -> String
    ) -> EvaluationChart {
        let days = rows.map { row -> ChartDay in
            let own = values(row)
            let total = own.reduce(0, +)
            return ChartDay(
                date: row.date, shortDate: JaFormat.shortDate(row.date), values: own, valueTexts: own.map(text),
                totalText: hasTotal ? text(total) : nil, spoken: spoken(JaFormat.shortDate(row.date), total)
            )
        }
        return EvaluationChart(
            title: title, hint: hint, kind: .columns, series: series, days: days, max: max, axis: axis,
            dateLabels: dateLabels(rows.map(\.date)), hasNoData: false
        )
    }

    static func lines(
        title: String, hint: String, series: [ChartSeries], dates: [String], values: [[Double?]], max: Double,
        axis: [String], text: (Double) -> String
    ) -> EvaluationChart {
        let days = dates.enumerated().map { index, date -> ChartDay in
            let own = values.map { $0[index] }
            let texts = own.map { $0.map(text) ?? Ja.Evaluation.noSamples }
            let spoken = zip(series, texts).map { "\($0.label) \($1)" }.joined(separator: "、")
            return ChartDay(date: date, shortDate: JaFormat.shortDate(date), values: own, valueTexts: texts, totalText: nil, spoken: "\(JaFormat.shortDate(date)): \(spoken)")
        }
        return EvaluationChart(
            title: title, hint: hint, kind: .lines, series: series, days: days, max: max, axis: axis,
            dateLabels: dateLabels(dates), hasNoData: values.allSatisfy { $0.allSatisfy { $0 == nil } }
        )
    }
}
