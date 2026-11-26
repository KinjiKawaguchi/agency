import AgencyAPI
import Foundation

/// A team's evaluation as the screen shows it: the headline numbers, then each of them over the days.
public struct TeamEvaluationReport: Equatable, Sendable, Identifiable {
    public struct Tile: Equatable, Sendable, Identifiable {
        public let label: String
        public let value: String
        public let note: String
        public var id: String { label }
    }

    /// The window of the lead time and cost trends, in days.
    public static let trendDays = 7

    public let team: String
    public let label: String
    public let tiles: [Tile]
    public let charts: [EvaluationChart]
    /// What each participant did, when the node gave it: none otherwise, and nothing is drawn for none.
    public let participants: ParticipantsSection?
    public var id: String { team }
    public var title: String { Ja.Evaluation.team(label) }

    /// - Parameter days: the period the person asked for, used when the team's own `from` and `until` cannot be read.
    public init(numbers: TeamEvaluation, label: String, days asked: Int) {
        let days = EvaluationSeries.dayCount(from: numbers.from, until: numbers.until) ?? asked
        team = numbers.team
        self.label = label
        tiles = Self.tiles(numbers.summary, days: days)
        charts = Self.charts(numbers, days: days)
        participants = ParticipantsSection(numbers: numbers)
    }

    // MARK: Tiles

    private static func oneDecimal(_ value: Double) -> String {
        let tenths = (value * 10).rounded() / 10
        return tenths == tenths.rounded() ? String(Int(tenths)) : String(tenths)
    }

    private static func tiles(_ summary: EvaluationSummary, days: Int) -> [Tile] {
        let merged = summary.merged.total + summary.reverts
        let perDay = { (count: Int) in Ja.Evaluation.Tile.perDay(oneDecimal(Double(count) / Double(max(days, 1)))) }
        func lead(_ label: String, _ median: Double?, _ p90: Double?) -> Tile {
            guard let median, let p90 else { return Tile(label: label, value: "—", note: Ja.Evaluation.noData) }
            return Tile(label: label, value: DurationFormat.text(median), note: Ja.Evaluation.Tile.leadNote(DurationFormat.text(p90)))
        }
        return [
            Tile(label: Ja.Evaluation.Tile.features, value: JaFormat.integer(summary.merged.feat), note: perDay(summary.merged.feat)),
            Tile(label: Ja.Evaluation.Tile.merged, value: JaFormat.integer(merged), note: perDay(merged)),
            lead(Ja.Evaluation.Tile.prLead, summary.prLeadMedianSeconds, summary.prLeadP90Seconds),
            lead(Ja.Evaluation.Tile.issueLead, summary.issueLeadMedianSeconds, summary.issueLeadP90Seconds),
            Tile(label: Ja.Evaluation.Tile.red, value: DurationFormat.text(summary.mainRedSeconds), note: Ja.Evaluation.Tile.redNote(DurationFormat.text(summary.longestRedSeconds))),
            Tile(label: Ja.Evaluation.Tile.reverts, value: JaFormat.integer(summary.reverts), note: Ja.Evaluation.Tile.revertsNote(merged)),
            summary.costPerMergedPrUsd.map { Tile(label: Ja.Evaluation.Tile.cost, value: JaFormat.money($0), note: Ja.Evaluation.Tile.costNote) }
                ?? Tile(label: Ja.Evaluation.Tile.cost, value: "—", note: Ja.Evaluation.noData),
        ]
    }

    // MARK: Charts

    /// The colors the pull request types keep: a change of period never repaints another type.
    private static func typeSeries() -> [ChartSeries] {
        MergedCounts.Kind.allCases.enumerated().map { index, kind in
            ChartSeries(key: kind.rawValue, label: Ja.Evaluation.kind(kind), slot: kind == .other ? nil : index + 1)
        }
    }

    private static func money(_ value: Double) -> String { value == 0 ? "$0" : JaFormat.money(value) }
    private static func count(_ value: Double) -> String { JaFormat.integer(Int(value.rounded())) }

    private static func charts(_ numbers: TeamEvaluation, days: Int) -> [EvaluationChart] {
        let rows = EvaluationSeries.fillDays(numbers.byDay, days: days, last: numbers.until)
        let dates = rows.map(\.date)
        let window = trendDays

        let mergedMax = EvaluationSeries.countMax(Double(rows.map { $0.merged.total }.max() ?? 0))
        let prLead = EvaluationSeries.rollingMedian(rows, window: window, of: \.prLeadSeconds)
        let issueLead = EvaluationSeries.rollingMedian(rows, window: window, of: \.issueLeadSeconds)
        let leadAxis = DurationFormat.axis(upTo: max(EvaluationSeries.secondsPerHour, (prLead + issueLead).compactMap { $0 }.max() ?? 0))
        let redAxis = DurationFormat.axis(upTo: max(EvaluationSeries.secondsPerHour, rows.map(\.mainRedSeconds).max() ?? 0))
        let revertsMax = EvaluationSeries.countMax(Double(rows.map(\.reverts).max() ?? 0))
        let costPerPr = EvaluationSeries.rollingCostPerPullRequest(rows, window: window)
        let costMax = EvaluationSeries.niceMax(costPerPr.compactMap { $0 }.max() ?? 0)
        let one = [ChartSeries(key: "one", label: "", slot: 1)]

        return [
            ChartBuilder.columns(
                title: Ja.Evaluation.Chart.merged, hint: Ja.Evaluation.Chart.mergedHint, series: typeSeries(), dates: dates,
                values: rows.map { row in MergedCounts.Kind.allCases.map { Double(row.merged[$0]) } }, max: mergedMax,
                axis: [mergedMax, mergedMax / 2, 0].map(count),
                text: count, hasTotal: true,
                spoken: { Ja.Evaluation.Chart.dayMerged($0, count($1)) }
            ),
            ChartBuilder.lines(
                title: Ja.Evaluation.Chart.lead, hint: Ja.Evaluation.Chart.leadHint,
                series: [
                    ChartSeries(key: "pr", label: Ja.Evaluation.Chart.prLead, slot: 1),
                    ChartSeries(key: "issue", label: Ja.Evaluation.Chart.issueLead, slot: 2),
                ],
                dates: dates, values: [prLead, issueLead], max: leadAxis.max,
                axis: [leadAxis.max, leadAxis.max / 2, 0].map(leadAxis.text), text: DurationFormat.text
            ),
            ChartBuilder.lines(
                title: Ja.Evaluation.Chart.cost, hint: Ja.Evaluation.Chart.costHint,
                series: [ChartSeries(key: "cost", label: Ja.Evaluation.Chart.cost, slot: 1)],
                dates: dates, values: [costPerPr], max: costMax, axis: [costMax, costMax / 2, 0].map(money), text: money
            ),
            ChartBuilder.columns(
                title: Ja.Evaluation.Chart.red, series: one, dates: dates, values: rows.map { [$0.mainRedSeconds] }, max: redAxis.max,
                axis: [redAxis.max, redAxis.max / 2, 0].map(redAxis.text), text: DurationFormat.text,
                hasTotal: false, spoken: { "\($0): \(DurationFormat.text($1))" }
            ),
            ChartBuilder.columns(
                title: Ja.Evaluation.Chart.reverts, series: one, dates: dates, values: rows.map { [Double($0.reverts)] }, max: revertsMax,
                axis: [revertsMax, revertsMax / 2, 0].map(count), text: count,
                hasTotal: false, spoken: { "\($0): \(count($1))" }
            ),
        ]
    }
}
