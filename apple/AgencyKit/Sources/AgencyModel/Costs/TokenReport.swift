import AgencyAPI
import Foundation

/// What a team's usage answer shows: three figures, the cost by day stacked by participant, and a row for each participant.
public struct TokenReport: Equatable, Sendable {
    public struct Tile: Equatable, Sendable, Identifiable {
        public let label: String
        public let value: String
        public let note: String
        public var id: String { label }
    }

    /// One participant's figures. A table of five columns would be wider than a phone, so each is a card.
    public struct Row: Equatable, Sendable, Identifiable {
        public let person: String
        public let label: String
        public let cost: String
        public let output: String
        public let cacheRead: String
        public let inputWrite: String
        public var id: String { person }
    }

    public let days: Int
    public let tiles: [Tile]
    public let series: [UsageChart.Series]
    public let columns: [UsageChart.Column]
    public let rows: [Row]

    public init(usage: TokenUsage, view: TeamView) {
        let labelOf: (String) -> String = { person in view.people.first { $0.id == person }?.label ?? person }
        let series = UsageChart.series(for: usage, teamOrder: view.people.map(\.id), labelOf: labelOf, otherLabel: Ja.Costs.other)
        let totals = UsageChart.totals(of: usage)
        days = usage.days
        self.series = series
        columns = UsageChart.columns(for: usage, series: series)
        tiles = [
            Tile(label: Ja.Costs.total(days: usage.days), value: JaFormat.money(totals.costUsd), note: Ja.Costs.average(JaFormat.money(totals.perDay))),
            Tile(label: Ja.Costs.output, value: JaFormat.compact(totals.output), note: Ja.Costs.inputSide(JaFormat.compact(totals.inputSide))),
            Tile(label: Ja.Costs.cacheShare, value: JaFormat.percent(totals.cacheShare * 100), note: Ja.Costs.cacheNote),
        ]
        rows = usage.byPerson.map { entry in
            Row(
                person: entry.person, label: labelOf(entry.person), cost: JaFormat.money(entry.costUsd),
                output: JaFormat.compact(entry.tokens.output), cacheRead: JaFormat.compact(entry.tokens.cacheRead),
                inputWrite: JaFormat.compact(entry.tokens.input + entry.tokens.cacheWrite)
            )
        }
    }
}
