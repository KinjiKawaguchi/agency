import Foundation
import Testing
@testable import AgencyAPI

@Suite("Outcomes wire")
struct OutcomesWireTests {
    private func decode<Value: Decodable>(_ type: Value.Type, _ json: String) throws -> Value {
        try JSONDecoder().decode(type, from: Data(json.utf8))
    }

    @Test("the shared sample of the numbers decodes: money in its own currency, a percent, and a KPI with no number")
    func outcomesSample() throws {
        let outcomes = try JSONDecoder().decode(Outcomes.self, from: Fixtures.sample("outcomes"))
        #expect(outcomes.org == "acme")
        #expect(outcomes.days == 14)
        #expect(outcomes.series.map(\.kpi.id) == ["revenue", "activation", "lead-time"])
        let revenue = outcomes.series[0]
        #expect(revenue.kpi.unit == .money(currency: "JPY"))
        #expect(revenue.kpi.kind == .business)
        #expect(revenue.kpi.goodWhen == .higher)
        #expect(revenue.before?.value == 1_200_000)
        #expect(revenue.points.map(\.value) == [1_350_000])
        #expect(revenue.points[0].note == "Before the month's last invoices")
        #expect(revenue.points[0].source == .manual)
        let activation = outcomes.series[1]
        #expect(activation.kpi.unit == .percent)
        #expect(activation.before == nil)
        #expect(activation.points.map(\.date) == ["2026-11-17", "2026-11-24"])
        #expect(activation.points[0].source == .connector(name: "analytics-export"))
        let leadTime = outcomes.series[2]
        #expect(leadTime.kpi.unit == .seconds)
        #expect(leadTime.kpi.goodWhen == .lower)
        #expect(leadTime.points.isEmpty && leadTime.before == nil, "a KPI with no number has none: not a 0")
    }

    @Test("the shared sample of the readings decodes, one for each state")
    func readingsSample() throws {
        let readings = try JSONDecoder().decode(Readings.self, from: Fixtures.sample("readings"))
        #expect(readings.readings.map(\.state) == [.ready, .ready, .notShipped, .shipDateUnknown, .noBaseline, .noFollowUp, .tooEarly, .tooFew])
        let first = readings.readings[0]
        #expect(first.epicTitle == "Persistent sessions")
        #expect(first.shippedOn == "2026-10-20")
        #expect(first.before?.value == 40 && first.after?.value == 44)
        #expect(first.change == 4 && first.changeRatio == 0.1)
        #expect(first.overlapping == ["app#12"])
        let notShipped = readings.readings[2]
        #expect(notShipped.before == nil && notShipped.after == nil && notShipped.change == nil, "no number but in ready")
        #expect(notShipped.overlapping.isEmpty)
        let tooFew = try #require(readings.readings.last)
        #expect(tooFew.points != nil && tooFew.needs != nil)
        let tooEarly = try #require(readings.readings.first { $0.state == .tooEarly })
        #expect(tooEarly.readableFrom != nil)
    }

    @Test("a unit, a direction, a kind or a state a newer node has is read as unknown, not refused")
    func unknowns() throws {
        let kpi = try decode(Kpi.self, #"{"id":"x","label":"X","kind":"market","unit":"basis-points","goodWhen":"sideways"}"#)
        #expect(kpi.kind == .unknown("market"))
        #expect(kpi.unit == .unknown("basis-points"))
        #expect(kpi.goodWhen == .unknown("sideways"))
        let reading = try decode(AimReading.self, #"{"team":"app","epic":"app#1","kpi":"x","state":"someday","basis":"correlation"}"#)
        #expect(reading.state == .unknown("someday"))
        #expect(reading.overlapping.isEmpty)
    }

    @Test("the windows the words name are the ones the node's reading module has")
    func windowsMatchTheNode() throws {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        let rust = try String(contentsOf: url.appendingPathComponent("crates/agency-proto/src/reading.rs"), encoding: .utf8)
        #expect(rust.contains("pub const SETTLE_DAYS: i64 = \(AimReading.settleDays);"))
        #expect(rust.contains("pub const WINDOW_DAYS: i64 = \(AimReading.windowDays);"))
    }

    @Test("a money KPI without its currency is refused: a number in no currency is not one to show")
    func moneyNeedsCurrency() {
        #expect(throws: DecodingError.self) {
            try decode(Kpi.self, #"{"id":"x","label":"X","kind":"business","unit":"money","goodWhen":"higher"}"#)
        }
    }
}
