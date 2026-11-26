import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

/// What the org's work cost on the days a KPI is drawn over, from the evaluation's per-team days.
enum CostSamples {
    /// An evaluation of teams that each report the days in `byDay` (a date and its cost) inside their own period, and `unavailable` teams
    /// that did not answer, built from the node's fixture.
    static func evaluation(teams: [(from: String, until: String, byDay: [(date: String, usd: Double)])], unavailable: Int = 0) throws -> Evaluation {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<6 { url.deleteLastPathComponent() }
        let data = try Data(contentsOf: url.appendingPathComponent("testdata/client/evaluation.json"))
        var root = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let team = try #require((root["teams"] as? [[String: Any]])?.first)
        let day = try #require((team["byDay"] as? [[String: Any]])?.first)
        root["teams"] = teams.enumerated().map { index, one -> [String: Any] in
            var copy = team
            copy["team"] = "team-\(index)"
            copy["from"] = one.from
            copy["until"] = one.until
            copy["byDay"] = one.byDay.map { entry -> [String: Any] in
                var made = day
                made["date"] = entry.date
                made["costUsd"] = entry.usd
                return made
            }
            return copy
        }
        root["unavailable"] = (0..<unavailable).map { ["team": "down-\($0)", "reason": "busy"] }
        return try JSONDecoder().decode(Evaluation.self, from: JSONSerialization.data(withJSONObject: root))
    }
}

@Suite("Cost on the days of a KPI")
struct CostOnDaysTests {
    private let dates = ["2026-11-20", "2026-11-21", "2026-11-22", "2026-11-23"]

    @Test("the days of a period are every calendar day from the first to the last, across a month and a leap day, and none for a bad range")
    func dayList() {
        #expect(OutcomeDay.dates(from: "2026-11-20", until: "2026-11-23") == dates)
        #expect(OutcomeDay.dates(from: "2026-11-29", until: "2026-12-02") == ["2026-11-29", "2026-11-30", "2026-12-01", "2026-12-02"])
        #expect(OutcomeDay.dates(from: "2028-02-28", until: "2028-03-01") == ["2028-02-28", "2028-02-29", "2028-03-01"])
        #expect(OutcomeDay.dates(from: "2026-11-23", until: "2026-11-20").isEmpty)
        #expect(OutcomeDay.dates(from: "yesterday", until: "2026-11-23").isEmpty)
        #expect(OutcomeDay.dates(from: "2025-01-01", until: "2026-11-23").isEmpty, "more than a year is not a period of a KPI")
        #expect(OutcomeDay.dates(from: "2026-11-20", until: "2026-11-20") == ["2026-11-20"])
    }

    @Test("a day is the sum over the teams whose period covers it, and a day inside a period that the node leaves out cost 0")
    func sums() throws {
        let cost = CostOnDays(evaluation: try CostSamples.evaluation(teams: [
            (from: "2026-11-20", until: "2026-11-23", byDay: [(date: "2026-11-21", usd: 4), (date: "2026-11-23", usd: 1.5)]),
            (from: "2026-11-20", until: "2026-11-23", byDay: [(date: "2026-11-21", usd: 2)]),
        ]), dates: dates)
        #expect(cost.days.map(\.usd) == [0, 6, 0, 1.5], "20 and 22 are in both periods and not in byDay")
    }

    @Test("a day inside no team's period has no cost, which is not a cost of 0, and a team is drawn against its own period")
    func uncovered() throws {
        let cost = CostOnDays(evaluation: try CostSamples.evaluation(teams: [
            (from: "2026-11-21", until: "2026-11-22", byDay: [(date: "2026-11-22", usd: 3)]),
        ]), dates: dates)
        #expect(cost.days.map(\.usd) == [nil, 0, 3, nil])
        #expect(cost.known.map(\.date) == ["2026-11-21", "2026-11-22"])
    }

    @Test("a team that did not answer is counted, and the sum is said to be of the others")
    func partial() throws {
        let cost = CostOnDays(evaluation: try CostSamples.evaluation(teams: [(from: "2026-11-20", until: "2026-11-23", byDay: [])], unavailable: 2), dates: dates)
        #expect(cost.teamsWithout == 2 && cost.partialLine == "報告できなかった 2 チームを除きます。")
        #expect(CostOnDays(evaluation: try CostSamples.evaluation(teams: []), dates: dates).partialLine == nil)
        #expect(CostOnDays(evaluation: try CostSamples.evaluation(teams: []), dates: dates).known.isEmpty)
    }

    @Test("a day says itself in words: its date and what it cost, or that nothing was reported, and the axis tops the dearest day")
    func words() throws {
        let cost = CostOnDays(evaluation: try CostSamples.evaluation(teams: [(from: "2026-11-21", until: "2026-11-23", byDay: [(date: "2026-11-23", usd: 9.55)])]), dates: dates)
        #expect(cost.describe(cost.days[3]) == "2026-11-23: $9.55")
        #expect(cost.describe(cost.days[0]) == "2026-11-20: コストの報告なし")
        #expect(cost.describe(cost.days[1]) == "2026-11-21: $0.00")
        #expect(cost.maximum >= 9.55 && cost.maximum < 20)
        #expect(Ja.Outcomes.Cost.title == "同じ日のコスト" && Ja.Outcomes.Cost.none == "この期間のコストは報告されていません。")
    }
}
