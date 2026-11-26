@testable import AgencyModel
import Testing

@Suite("Trend layout")
struct TrendLayoutTests {
    private func points(_ pairs: [(String, Double)]) -> [KpiCard.Point] {
        let series = try! OutcomeSamples.series(unit: "count", points: pairs)
        return KpiCard(series: series).points
    }

    @Test("a number sits across by its day, so a day without a number is a gap")
    func across() {
        let layout = TrendLayout(points: points([("2026-11-01", 10), ("2026-11-02", 20), ("2026-11-21", 30)]), from: "2026-11-01", until: "2026-11-21")
        #expect(layout.spots.map(\.x) == [0, 0.05, 1])
        #expect(layout.spots.map(\.y) == [0, 0.5, 1])
    }

    @Test("a single number, or numbers that are all the same, sit in the middle height")
    func flat() {
        let single = TrendLayout(points: points([("2026-11-11", 7)]), from: "2026-11-01", until: "2026-11-21")
        #expect(single.spots == [TrendLayout.Spot(x: 0.5, y: 0.5)])
        let same = TrendLayout(points: points([("2026-11-01", 5), ("2026-11-21", 5)]), from: "2026-11-01", until: "2026-11-21")
        #expect(same.spots.map(\.y) == [0.5, 0.5])
    }

    @Test("no numbers gives no spots, never a line at 0")
    func empty() {
        #expect(TrendLayout(points: [], from: "2026-11-01", until: "2026-11-21").spots.isEmpty)
    }

    @Test("a number outside the period is held at its edge, and a day that cannot be read is left out")
    func edges() {
        let layout = TrendLayout(points: points([("2026-10-30", 1), ("2026-11-30", 2)]), from: "2026-11-01", until: "2026-11-21")
        #expect(layout.spots.map(\.x) == [0, 1])
        #expect(TrendLayout(points: points([("2026-11-05", 1)]), from: "soon", until: "later").spots.isEmpty)
    }

    @Test("a one-day period puts a number in the middle")
    func oneDay() {
        let layout = TrendLayout(points: points([("2026-11-01", 1)]), from: "2026-11-01", until: "2026-11-01")
        #expect(layout.spots.map(\.x) == [0.5])
    }
}
