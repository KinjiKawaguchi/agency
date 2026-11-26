import AgencyAPI
@testable import AgencyModel
import Testing

@Suite("KPI card")
struct KpiCardTests {
    private let samples = try! OutcomeSamples.outcomes()

    @Test("revenue: the latest number in its own currency, as of its day, and the movement from the baseline, better because higher is better")
    func revenue() throws {
        let card = KpiCard(series: samples.series[0])
        guard case .latest(let text, let day) = card.figure else { Issue.record("no latest number"); return }
        #expect(text.contains("1,350,000"))
        #expect(day == "2026年11月20日")
        let movement = try #require(card.movement)
        #expect(movement.direction == .rose && movement.tone == .better)
        #expect(movement.change.hasPrefix("+") && movement.change.contains("150,000"))
        #expect(movement.ratio == "+12.5%")
        #expect(movement.text.hasPrefix("上がった +") && movement.text.contains("150,000") && movement.text.hasSuffix("（+12.5%）良い方"))
        #expect(card.points == [KpiCard.Point(day: "2026-11-20", value: 1_350_000)])
    }

    @Test("with the period's last day the days of the same year are the web's short date")
    func shortDays() {
        let card = KpiCard(series: samples.series[0], until: samples.until)
        guard case .latest(_, let day) = card.figure else { Issue.record("no latest number"); return }
        #expect(day == "11月20日")
    }

    @Test("activation: two numbers and no baseline, so a latest number and no movement")
    func activation() {
        let card = KpiCard(series: samples.series[1])
        #expect(card.figure == .latest(text: "43.2%", day: "2026年11月24日"))
        #expect(card.movement == nil, "a movement is read from the baseline, and there is none")
        #expect(card.points.map(\.value) == [41.5, 43.2])
    }

    @Test("a KPI with no number at all has a gap, never a 0")
    func noNumber() {
        let card = KpiCard(series: samples.series[2])
        #expect(card.figure == .none)
        #expect(card.movement == nil)
        #expect(card.points.isEmpty)
    }

    @Test("a KPI with no number in the period but one before it shows that one as older, with no movement")
    func older() throws {
        let series = try OutcomeSamples.series(unit: "count", before: 40, points: [])
        let card = KpiCard(series: series)
        #expect(card.figure == .older(text: "40", day: "2026年10月31日"))
        #expect(card.movement == nil)
    }

    @Test("lower is better: a fall is better and a rise is worse; unchanged and an unknown direction colour nothing")
    func tone() throws {
        func movement(goodWhen: String, before: Double, latest: Double) throws -> Movement {
            let series = try OutcomeSamples.series(unit: "count", goodWhen: goodWhen, before: before, points: [("2026-11-20", latest)])
            return try #require(KpiCard(series: series).movement)
        }
        let fell = try movement(goodWhen: "lower", before: 10, latest: 6)
        #expect(fell.direction == .fell && fell.tone == .better && fell.text == "下がった -4（-40%）良い方")
        let rose = try movement(goodWhen: "lower", before: 10, latest: 12)
        #expect(rose.direction == .rose && rose.tone == .worse && rose.text == "上がった +2（+20%）悪い方")
        let fellHigher = try movement(goodWhen: "higher", before: 10, latest: 6)
        #expect(fellHigher.tone == .worse)
        let same = try movement(goodWhen: "higher", before: 10, latest: 10)
        #expect(same.direction == .unchanged && same.tone == .neutral && same.text == "変わらず")
        let unknown = try movement(goodWhen: "sideways", before: 10, latest: 12)
        #expect(unknown.direction == .rose && unknown.tone == .neutral && unknown.text == "上がった +2（+20%）", "no tag, so no colour")
    }

    @Test("a latest number of 0 is a number: it is drawn as 0, and is not the gap")
    func zeroIsANumber() throws {
        let series = try OutcomeSamples.series(unit: "percent", points: [("2026-11-20", 0)])
        #expect(KpiCard(series: series).figure == .latest(text: "0%", day: "2026年11月20日"))
    }
}
