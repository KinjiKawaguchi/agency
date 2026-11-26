@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

/// The name of the node's recorded usage answer in `testdata/client`.
let usageFixture = "usage"

/// Ported from `web/src/costs/usage.test.ts`.
@Suite("Usage chart")
struct UsageChartTests {
    private let tokens = #"{"input":10,"output":20,"cacheRead":80,"cacheWrite":10}"#

    private func report(
        byDay: String = #"[{"date":"2026-11-24","costUsd":{"lead":4,"backend":6}},{"date":"2026-11-25","costUsd":{"lead":1,"backend":2,"ghost":3}}]"#,
        byPerson: [(String, Double)] = [("backend", 8), ("lead", 5), ("ghost", 3)]
    ) -> TokenUsage {
        let people = byPerson.map { #"{"person":"\#($0.0)","costUsd":\#($0.1),"tokens":\#(tokens)}"# }.joined(separator: ",")
        let json = #"{"org":"acme","team":"app","days":2,"generatedAt":"2026-11-25T00:00:00Z","byDay":\#(byDay),"byPerson":[\#(people)]}"#
        return try! JSONDecoder().decode(TokenUsage.self, from: Data(json.utf8))
    }

    private func label(_ person: String) -> String { person.uppercased() }

    @Test("series follow the team's order and give each participant a fixed color slot")
    func order() {
        let series = UsageChart.series(for: report(), teamOrder: ["lead", "backend", "frontend"], labelOf: label, otherLabel: "Other")
        #expect(series.map(\.person) == ["lead", "backend", "ghost"])
        #expect(series.map(\.slot) == [1, 2, 3])
        #expect(series.map(\.label) == ["LEAD", "BACKEND", "GHOST"])
    }

    @Test("participants past the eighth are folded into one other series instead of a ninth hue")
    func folds() {
        let many = report(byPerson: (0..<(UsageChart.maxSeries + 2)).map { ("p\($0)", 1.0) })
        let series = UsageChart.series(for: many, teamOrder: [], labelOf: label, otherLabel: "Other")
        #expect(series.count == UsageChart.maxSeries + 1)
        #expect(series.last == UsageChart.Series(person: nil, label: "Other", slot: nil))
    }

    @Test("exactly eight participants each have a color of their own, and nothing is folded")
    func eight() {
        let eight = report(byPerson: (0..<UsageChart.maxSeries).map { ("p\($0)", 1.0) })
        let series = UsageChart.series(for: eight, teamOrder: [], labelOf: label, otherLabel: "Other")
        #expect(series.count == UsageChart.maxSeries)
        #expect(series.allSatisfy { $0.slot != nil })
    }

    @Test("the cost of each series for each day, oldest first")
    func columns() {
        let usage = report()
        let series = UsageChart.series(for: usage, teamOrder: ["lead", "backend"], labelOf: label, otherLabel: "Other")
        let columns = UsageChart.columns(for: usage, series: series)
        #expect(columns.map(\.date) == ["2026-11-24", "2026-11-25"])
        #expect(columns.map(\.values) == [[4, 6, 0], [1, 2, 3]])
        #expect(columns.map(\.total) == [10, 6])
    }

    @Test("the folded participants are added into the other column")
    func otherColumn() {
        let series = [
            UsageChart.Series(person: "lead", label: "LEAD", slot: 1),
            UsageChart.Series(person: nil, label: "Other", slot: nil),
        ]
        #expect(UsageChart.columns(for: report(), series: series)[1].values == [1, 5])
    }

    @Test("the participants add up, and the cache share is read from the input side")
    func totals() {
        let totals = UsageChart.totals(of: report())
        #expect(totals.costUsd == 16)
        #expect(totals.perDay == 8)
        #expect(totals.output == 60)
        #expect(totals.inputSide == 300)
        #expect(abs(totals.cacheShare - 0.8) < 0.0001)
    }

    @Test("nothing used divides by nothing")
    func nothing() {
        let totals = UsageChart.totals(of: report(byDay: "[]", byPerson: []))
        #expect(totals == UsageChart.Totals(costUsd: 0, perDay: 0, output: 0, inputSide: 0, cacheShare: 0))
    }

    @Test("the node's recorded answer adds up to the cost per participant and per day it states")
    func recorded() {
        let shared = Fixture.decode(TokenUsage.self, usageFixture)
        #expect(abs(UsageChart.totals(of: shared).costUsd - 13.3) < 0.0001)
        let series = UsageChart.series(for: shared, teamOrder: ["lead", "backend"], labelOf: { $0 }, otherLabel: "Other")
        #expect(series.map(\.person) == ["lead", "backend", "reviewer"])
        let daily = UsageChart.columns(for: shared, series: series).map(\.total)
        #expect(abs(daily[0] - 9.55) < 0.0001)
        #expect(abs(daily[1] - 3.75) < 0.0001)
    }

    @Test("compact numbers and money read as the web client writes them")
    func format() {
        #expect(JaFormat.compact(999) == "999")
        #expect(JaFormat.compact(1_234) == "1.2K")
        #expect(JaFormat.compact(2_400_000) == "2.4M")
        #expect(JaFormat.compact(5_670_000_000) == "5.67B")
    }
}

@Suite("Token report")
struct TokenReportTests {
    private let team = Teams.team()

    @Test("three figures, and a row for each participant with the team's own label")
    func report() {
        let report = TokenReport(usage: Fixture.decode(TokenUsage.self, usageFixture), view: team.view)
        #expect(report.days == 7)
        #expect(report.tiles.map(\.label) == ["直近 7 日", "出力トークン", "キャッシュから読んだ入力"])
        #expect(report.tiles[0].value == "$13")
        #expect(report.tiles[0].note == "1 日平均 $6.65")
        #expect(report.tiles[1].value == "69.0K")
        #expect(report.rows.map(\.label) == ["Backend", "Lead", "reviewer"])
        #expect(report.rows[0].cost == "$8.45")
        #expect(report.rows[0].cacheRead == "2.4M")
        #expect(report.rows[0].inputWrite == "300.0K")
        #expect(report.series.map(\.person) == ["lead", "backend", "reviewer"])
    }
}
