import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

@Suite("Number history")
struct NumberHistoryTests {
    private func entry(_ date: String, _ value: Double, _ enteredAt: String, kpi: String = "revenue", source: String = #"{"kind":"manual"}"#, note: String? = nil) throws -> KpiObservation {
        let noteField = note.map { #","note":"\#($0)""# } ?? ""
        let json = #"{"kpi":"\#(kpi)","date":"\#(date)","value":\#(value),"source":\#(source),"enteredAt":"\#(enteredAt)","enteredBy":"owner"\#(noteField)}"#
        return try JSONDecoder().decode(KpiObservation.self, from: Data(json.utf8))
    }

    @Test("the earlier entry of a day entered twice is marked replaced, and the later one is not")
    func replaced() throws {
        let history = [try entry("2026-11-20", 2, "2026-11-22T09:00:00Z"), try entry("2026-11-19", 1, "2026-11-20T09:00:00Z"), try entry("2026-11-20", 1, "2026-11-21T09:00:00Z")]
        #expect(NumberHistoryEntry.replacedIn(history) == [false, false, true])
    }

    @Test("KPIs are not mixed up, and nothing is marked when each day was entered once")
    func notMixed() throws {
        #expect(NumberHistoryEntry.replacedIn([try entry("2026-11-20", 1, "2026-11-21T09:00:00Z"), try entry("2026-11-20", 2, "2026-11-20T09:00:00Z", kpi: "other")]) == [false, false])
        #expect(NumberHistoryEntry.replacedIn([]).isEmpty)
    }

    @Test("times are compared as instants, not as text: a fraction of a second or another zone does not change which is later")
    func instants() throws {
        // As text "…:05Z" sorts after "…:05.5Z" ('Z' > '.'), but the second is the later instant.
        let fraction = [try entry("2026-11-20", 1, "2026-11-21T09:00:05Z"), try entry("2026-11-20", 2, "2026-11-21T09:00:05.5Z")]
        #expect(NumberHistoryEntry.replacedIn(fraction) == [true, false])
        let zone = [try entry("2026-11-20", 1, "2026-11-21T09:00:00Z"), try entry("2026-11-20", 2, "2026-11-21T09:30:00+09:00")]
        #expect(NumberHistoryEntry.replacedIn(zone) == [false, true], "09:30 at +09:00 is 00:30Z, earlier")
    }

    @Test("an entry shows its day, its number by the unit, where it came from, when it was entered, and its note")
    func listed() throws {
        let tokyo = TimeZone(identifier: "Asia/Tokyo")!
        let list = NumberHistoryEntry.list(
            [try entry("2026-11-20", 1_350_000, "2026-11-21T09:00:00Z", note: "Before the invoices"),
             try entry("2026-11-17", 41.5, "2026-11-18T03:00:00Z", source: #"{"kind":"connector","name":"analytics-export"}"#)],
            unit: .percent, until: "2026-11-24", timeZone: tokyo
        )
        #expect(list[0].day == "11月20日" && list[0].value == "1,350,000%")
        #expect(list[0].source == "手で入れた数値" && list[0].enteredOn == "11月21日 18:00 に入力" && list[0].note == "Before the invoices")
        #expect(list[1].source == "analytics-export から" && list[1].note == nil && !list[1].isReplaced)
    }

    @Test("the ids are distinct even for two entries of one day and one time")
    func ids() throws {
        let same = [try entry("2026-11-20", 1, "2026-11-21T09:00:00Z"), try entry("2026-11-20", 2, "2026-11-21T09:00:00Z")]
        let list = NumberHistoryEntry.list(same, unit: .count, until: "2026-11-24")
        #expect(Set(list.map(\.id)).count == 2)
    }
}
