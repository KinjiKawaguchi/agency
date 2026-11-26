import AgencyAPI
@testable import AgencyModel
import Testing

@Suite("Outcome format")
struct OutcomeFormatTests {
    @Test("money keeps its own currency: yen without decimals, dollars with cents, never converted")
    func money() {
        let yen = OutcomeFormat.value(1_350_000, unit: .money(currency: "JPY"))
        #expect(yen.contains("1,350,000") && !yen.contains("."))
        #expect(!yen.contains("$"))
        let dollars = OutcomeFormat.value(1234.5, unit: .money(currency: "USD"))
        #expect(dollars.contains("1,234.50") && dollars.contains("$"))
        #expect(OutcomeFormat.value(1000, unit: .money(currency: "EUR")).contains("€"))
        #expect(OutcomeFormat.value(1000, unit: .money(currency: "JPY")) != OutcomeFormat.value(1000, unit: .money(currency: "USD")))
    }

    @Test("a percent is the number as it is, never clipped to 0 to 100")
    func percent() {
        #expect(OutcomeFormat.value(41.5, unit: .percent) == "41.5%")
        #expect(OutcomeFormat.value(120, unit: .percent) == "120%")
        #expect(OutcomeFormat.value(-3, unit: .percent) == "-3%")
        #expect(OutcomeFormat.value(0, unit: .percent) == "0%", "a 0 the node gave is a 0")
    }

    @Test("a count is grouped, a ratio has its cross, a duration is written as the evaluation writes it")
    func others() {
        #expect(OutcomeFormat.value(12_345, unit: .count) == "12,345")
        #expect(OutcomeFormat.value(1.5, unit: .ratio) == "1.5×")
        #expect(OutcomeFormat.value(0.8, unit: .ratio) == "0.8×")
        #expect(OutcomeFormat.value(90 * 60, unit: .seconds) == DurationFormat.text(90 * 60))
        #expect(OutcomeFormat.value(3.25, unit: .unknown("basis-points")) == "3.25", "an unknown unit is a plain number")
    }

    @Test("a change has its sign, a percent's in points, and no change is 0")
    func change() {
        #expect(OutcomeFormat.change(4, unit: .percent) == "+4ポイント")
        #expect(OutcomeFormat.change(-1.5, unit: .percent) == "-1.5ポイント")
        #expect(OutcomeFormat.change(150_000, unit: .money(currency: "JPY")).hasPrefix("+"))
        #expect(OutcomeFormat.change(-150_000, unit: .money(currency: "JPY")).hasPrefix("-"))
        #expect(OutcomeFormat.change(500, unit: .count) == "+500")
        #expect(OutcomeFormat.change(-0.5, unit: .ratio) == "-0.5×")
        #expect(OutcomeFormat.change(3600, unit: .seconds) == "+" + DurationFormat.text(3600))
        #expect(OutcomeFormat.change(-3600, unit: .seconds) == "-" + DurationFormat.text(3600))
        #expect(OutcomeFormat.change(0, unit: .percent) == "0%")
    }

    @Test("a day is read in Japanese, and anything that is not a day is left as it came")
    func day() {
        #expect(OutcomeFormat.day("2026-11-20") == "2026年11月20日")
        #expect(OutcomeFormat.day("2026-01-05") == "2026年1月5日")
        #expect(OutcomeFormat.day("soon") == "soon")
    }

    @Test("a day in the year of the reference is the web's short date, and one in another year keeps its year")
    func shortDay() {
        #expect(OutcomeFormat.day("2026-11-20", relativeTo: "2026-11-24") == "11月20日")
        #expect(OutcomeFormat.day("2025-12-03", relativeTo: "2026-11-24") == "2025年12月3日")
        #expect(OutcomeFormat.day("2026-11-20", relativeTo: "soon") == "2026年11月20日", "no reference year, so the year is kept")
    }

    @Test("a change as a share of where it began has its sign, and none when it began at 0")
    func ratio() {
        #expect(OutcomeFormat.changeRatio(from: 40, to: 44) == "+10%")
        #expect(OutcomeFormat.changeRatio(from: 1_200_000, to: 1_350_000) == "+12.5%")
        #expect(OutcomeFormat.changeRatio(from: 10, to: 6) == "-40%")
        #expect(OutcomeFormat.changeRatio(from: -10, to: -5) == "+50%", "the share of a negative start is of its size")
        #expect(OutcomeFormat.changeRatio(from: 0, to: 5) == nil)
    }
}
