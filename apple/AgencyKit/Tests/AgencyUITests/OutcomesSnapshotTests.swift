#if canImport(SwiftUI) && os(macOS)
import AgencyAPI
import AgencyModel
import SwiftUI
import Testing
@testable import AgencyUI

/// The outcomes page drawn at phone width from the shared samples (made-up figures). The rule checked is the owner's:
/// nothing is wider than the screen. With `AGENCY_SNAPSHOT_DIR` set, each test leaves a PNG to look at.
@MainActor
@Suite("Outcomes renderings", .serialized)
struct OutcomesSnapshotTests {
    private let width = Snapshot.phoneWidth

    private func sample<Value: Decodable>(_ type: Value.Type, _ name: String, from file: String = #filePath) -> Value {
        var url = URL(fileURLWithPath: file)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        let data = try! Data(contentsOf: url.appendingPathComponent("testdata/\(name).sample.json"))
        return try! JSONDecoder().decode(type, from: data)
    }

    private func expectFits(_ result: Snapshot.Result, _ name: String, sourceLocation: SourceLocation = #_sourceLocation) {
        #expect(!result.png.isEmpty, "\(name) was not drawn", sourceLocation: sourceLocation)
        #expect(result.size.width == width, "\(name) is \(result.size.width) points wide, the screen is \(width)", sourceLocation: sourceLocation)
        #expect(result.size.height > 40, sourceLocation: sourceLocation)
    }

    private func orgs(readings: OrgReadings? = nil) -> [OrgOutcomes] {
        let outcomes = sample(Outcomes.self, "outcomes")
        let readingList = sample(Readings.self, "readings")
        let kpis = Dictionary(uniqueKeysWithValues: outcomes.series.map { ($0.kpi.id, $0.kpi) })
        let lines = readingList.readings.map { ReadingLine(reading: $0, team: "App", kpi: kpis[$0.kpi], today: "2026-11-24") }
        return [OrgOutcomes(
            id: "acme", label: "Acme", from: outcomes.from, until: outcomes.until,
            cards: outcomes.series.map { KpiCard(series: $0, until: outcomes.until) }, readings: readings ?? .lines(lines)
        )]
    }

    private func series(_ json: String) -> KpiSeries { try! JSONDecoder().decode(KpiSeries.self, from: Data(json.utf8)) }

    private func observation(_ day: String, _ value: Double) -> String {
        #"{"kpi":"k","date":"\#(day)","value":\#(value),"source":{"kind":"manual"},"enteredAt":"2026-11-24T12:00:00Z","enteredBy":"owner"}"#
    }

    // MARK: Renderings

    @Test("the sample: money in yen that rose, a percent, a KPI with no number, and a reading for each state")
    func sampleOrg() {
        for scheme in [ColorScheme.light, .dark] {
            let result = Snapshot.capture("outcomes", OutcomesContent(orgs: orgs(), days: 90), scheme: scheme)
            expectFits(result, "the sample, \(scheme)")
        }
    }

    @Test("a long label, a long epic title, a worse movement, an unknown direction, and a number older than the period")
    func stress() {
        let long = UIFixtures.longJapaneseTitle
        let worse = series(
            #"{"kpi":{"id":"k1","label":"\#(long)","kind":"product","unit":"seconds","goodWhen":"lower"},"points":["#
                + observation("2026-11-18", 7200) + "," + observation("2026-11-24", 90_000) + #"],"before":"# + observation("2026-10-31", 3600) + "}"
        )
        let unknown = series(
            #"{"kpi":{"id":"k2","label":"Basis points","kind":"product","unit":"basis-points","goodWhen":"sideways"},"points":["#
                + observation("2026-11-24", 12.5) + #"],"before":"# + observation("2026-10-31", 10) + "}"
        )
        let older = series(#"{"kpi":{"id":"k3","label":"Gross margin","kind":"business","unit":"ratio","goodWhen":"higher"},"points":[],"before":"# + observation("2026-10-31", 1.5) + "}")
        let dollars = series(
            #"{"kpi":{"id":"k4","label":"ARR","kind":"business","unit":"money","currency":"USD","goodWhen":"higher"},"points":["#
                + observation("2026-11-24", 1_234_567.5) + #"],"before":"# + observation("2026-10-31", 1_300_000) + "}"
        )
        let lines = [
            ReadingLine(reading: sample(Readings.self, "readings").readings[0], team: UIFixtures.unbrokenID, kpi: nil),
        ]
        let org = OrgOutcomes(
            id: "acme", label: "Acme", from: "2026-08-27", until: "2026-11-24",
            cards: [worse, unknown, older, dollars].map { KpiCard(series: $0, until: "2026-11-24") }, readings: .lines(lines)
        )
        for scheme in [ColorScheme.light, .dark] {
            expectFits(Snapshot.capture("outcomes-stress", OutcomesContent(orgs: [org], days: 30), scheme: scheme), "stress, \(scheme)")
        }
    }

    @Test("the buttons to enter a number and read the history, an open history with a replaced entry, and one that is loading or failed")
    func historyAndEnter() throws {
        func entry(_ date: String, _ value: Double, _ at: String) -> KpiObservation {
            try! JSONDecoder().decode(KpiObservation.self, from: Data(#"{"kpi":"revenue","date":"\#(date)","value":\#(value),"source":{"kind":"manual"},"enteredAt":"\#(at)","enteredBy":"owner"}"#.utf8))
        }
        let revenue = orgs()[0].cards[0]
        let list = NumberHistoryEntry.list(
            [entry("2026-11-20", 1_350_000, "2026-11-22T09:00:00Z"), entry("2026-11-20", 1_300_000, "2026-11-21T09:00:00Z"),
             entry("2026-10-31", 1_200_000, "2026-11-02T09:00:00Z")],
            unit: revenue.unit, until: "2026-11-24", timeZone: TimeZone(identifier: "Asia/Tokyo")!
        )
        #expect(list.map(\.isReplaced) == [false, true, false])
        let key = OutcomesStore.historyKey(org: "acme", kpi: "revenue")
        for (name, state) in [("open", HistoryState.entries(list)), ("loading", .loading), ("failed", .failed("down"))] {
            let content = OutcomesContent(
                orgs: orgs(), days: 90, histories: [key: state], onEnter: { _, _ in }, onToggleHistory: { _, _ in }
            )
            for scheme in [ColorScheme.light, .dark] {
                expectFits(Snapshot.capture("outcomes-history-\(name)", content, scheme: scheme), "history \(name), \(scheme)")
            }
        }
    }

    @Test("the day picked in the sheet is the day sent: a calendar day carried in UTC comes back as it went in")
    func sheetDay() {
        for day in ["2026-11-24", "2016-11-24", "2026-03-29", "2026-02-28", "2024-02-29", "2026-12-31"] {
            #expect(EnterNumberSheet.text(of: EnterNumberSheet.date(of: day)!) == day, Comment(rawValue: day))
        }
        #expect(EnterNumberSheet.date(of: "soon") == nil)
    }

    @Test("an org with no KPIs, no aims, and readings that could not be read")
    func empty() {
        let none = OrgOutcomes(id: "acme", label: "Acme", from: "2026-08-27", until: "2026-11-24", cards: [], readings: .lines([]))
        expectFits(Snapshot.capture("outcomes-empty", OutcomesContent(orgs: [none], days: 90)), "no KPIs")
        let failed = orgs(readings: .failed("the readings could not be read"))
        expectFits(Snapshot.capture("outcomes-readings-failed", OutcomesContent(orgs: failed, days: 90), scheme: .dark), "readings failed")
        let two = orgs() + [OrgOutcomes(id: "lab", label: "Lab", from: "2026-08-27", until: "2026-11-24", cards: [], readings: .unavailable)]
        expectFits(Snapshot.capture("outcomes-two-orgs", OutcomesContent(orgs: two, days: 90)), "two orgs")
    }
}
#endif
