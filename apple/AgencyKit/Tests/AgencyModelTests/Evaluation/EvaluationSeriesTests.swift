@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

/// Ported from `web/src/evaluation/series.test.ts` and `duration.test.ts`.
@Suite("Evaluation series")
struct EvaluationSeriesTests {
    private func day(_ date: String, merged: MergedCounts = MergedCounts(), reverts: Int = 0, cost: Double = 0, prLead: [Double] = []) -> DayEvaluation {
        DayEvaluation(date: date, merged: merged, reverts: reverts, costUsd: cost, prLeadSeconds: prLead)
    }

    @Test("lists every day of the period, oldest first, with the days the node left out empty")
    func fill() {
        let rows = EvaluationSeries.fillDays([day("2026-11-23", reverts: 1), day("2026-11-26", reverts: 2)], days: 5, last: "2026-11-26")
        #expect(rows.map(\.date) == ["2026-11-22", "2026-11-23", "2026-11-24", "2026-11-25", "2026-11-26"])
        #expect(rows.map(\.reverts) == [0, 1, 0, 0, 2])
    }

    @Test("crosses a month end and a year end")
    func crossing() {
        #expect(EvaluationSeries.fillDays([], days: 3, last: "2027-01-01").map(\.date) == ["2026-12-30", "2026-12-31", "2027-01-01"])
        #expect(EvaluationSeries.daysEnding("2026-03-02", days: 4) == ["2026-02-27", "2026-02-28", "2026-03-01", "2026-03-02"])
        #expect(EvaluationSeries.daysEnding("2028-03-01", days: 2) == ["2028-02-29", "2028-03-01"])
    }

    @Test("a last day that is not a day gives no days, and a period of none gives none")
    func notADay() {
        #expect(EvaluationSeries.daysEnding("soon", days: 3).isEmpty)
        #expect(EvaluationSeries.daysEnding("2026-11-26", days: 0).isEmpty)
    }

    @Test("the days of a period are counted from its first to its last, both included")
    func dayCount() {
        #expect(EvaluationSeries.dayCount(from: "2026-11-11", until: "2026-11-24") == 14)
        #expect(EvaluationSeries.dayCount(from: "2026-11-24", until: "2026-11-24") == 1)
        #expect(EvaluationSeries.dayCount(from: "2026-12-30", until: "2027-01-02") == 4)
        #expect(EvaluationSeries.dayCount(from: "2026-11-24", until: "2026-11-11") == nil)
        #expect(EvaluationSeries.dayCount(from: "soon", until: "2026-11-11") == nil)
    }

    @Test("the median is the middle value, the mean of the two middle ones, or nothing")
    func median() {
        #expect(EvaluationSeries.median([5, 1, 3]) == 3)
        #expect(EvaluationSeries.median([4, 1, 3, 2]) == 2.5)
        #expect(EvaluationSeries.median([]) == nil)
    }

    @Test("a rolling median pools every lead time of the days in the trailing window")
    func rolling() {
        let rows = [day("a", prLead: [10]), day("b"), day("c", prLead: [30, 50]), day("d"), day("e"), day("f", prLead: [90])]
        #expect(EvaluationSeries.rollingMedian(rows, window: 3, of: \.prLeadSeconds) == [10, 10, 30, 40, 40, 90])
        #expect(EvaluationSeries.rollingMedian([day("a"), day("b")], window: 2, of: \.prLeadSeconds) == [nil, nil])
    }

    @Test("cost per merged pull request divides the window's cost by its merged pull requests, reverts counted, and says nothing when none merged")
    func costPerPullRequest() {
        let rows = [day("a", cost: 10), day("b", merged: MergedCounts(feat: 2, fix: 1), cost: 20), day("c", cost: 30)]
        let result = EvaluationSeries.rollingCostPerPullRequest(rows, window: 2)
        #expect(result[0] == nil)
        #expect(result[1] == 10)
        #expect(abs((result[2] ?? 0) - 50.0 / 3) < 0.0001)
        // A pull request that reverts another was merged too: the node divides by both.
        let withRevert = [day("a", merged: MergedCounts(feat: 1), reverts: 1, cost: 8)]
        #expect(EvaluationSeries.rollingCostPerPullRequest(withRevert, window: 7) == [4])
        #expect(EvaluationSeries.pullRequestsMerged(withRevert[0]) == 2)
    }

    @Test("axis tops round up to 1, 2 or 5 times a power of ten, and counts stay whole in the middle too")
    func axes() {
        #expect([0, 0.8, 1.2, 3, 7, 120, 5000].map(EvaluationSeries.niceMax) == [1, 1, 2, 5, 10, 200, 5000])
        #expect([0, 1, 2, 3, 5, 7, 11].map(EvaluationSeries.countMax) == [2, 2, 2, 6, 6, 10, 20])
    }
}

@Suite("Durations")
struct DurationFormatTests {
    @Test("minutes under an hour, hours under two days, days above")
    func text() {
        #expect(DurationFormat.text(0) == "0分")
        #expect(DurationFormat.text(45 * 60) == "45分")
        #expect(DurationFormat.text(5400) == "1.5時間")
        #expect(DurationFormat.text(12 * 3600) == "12時間")
        #expect(DurationFormat.text(36 * 3600) == "36時間")
        #expect(DurationFormat.text(3 * 86_400) == "3日")
        #expect(DurationFormat.text(5 * 86_400 + 43_200) == "5.5日")
    }

    @Test("an axis tops out at a round number of one unit and writes every tick in it")
    func axis() {
        let hours = DurationFormat.axis(upTo: 5.5 * 3600)
        #expect(hours.max == 10 * 3600)
        #expect([hours.max, hours.max / 2, 0].map(hours.text) == ["10時間", "5時間", "0時間"])
        let days = DurationFormat.axis(upTo: 3 * 86_400)
        #expect(days.max == 5 * 86_400)
        #expect(days.text(days.max / 2) == "2.5日")
        let minutes = DurationFormat.axis(upTo: 20 * 60)
        #expect(minutes.max == 20 * 60)
        #expect(minutes.text(0) == "0分")
    }

    @Test("an axis up to exactly two days is still in hours, and one past it is in days")
    func twoDays() {
        #expect(DurationFormat.axis(upTo: 2 * 86_400).max == 50 * 3600)
        #expect(DurationFormat.axis(upTo: 2 * 86_400 + 1).max == 5 * 86_400)
        #expect(DurationFormat.axis(upTo: 3599).text(1800) == "30分")
        #expect(DurationFormat.axis(upTo: 3600).text(0) == "0時間")
    }

    @Test("there is still an axis when there is nothing to draw")
    func empty() {
        #expect(DurationFormat.axis(upTo: 0).max > 0)
    }
}
