@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

extension TeamEvaluation {
    /// The same numbers over another period, for a test.
    static func with(from: String, until: String, of team: TeamEvaluation) -> TeamEvaluation {
        let json = #"{"team":"\#(team.team)","from":"\#(from)","until":"\#(until)","summary":{"merged":{"feat":3},"mainRedSeconds":0,"longestRedSeconds":0,"reverts":0},"byDay":[]}"#
        return try! JSONDecoder().decode(TeamEvaluation.self, from: Data(json.utf8))
    }
}

/// The shared sample (`testdata/evaluation.sample.json`) drawn as the screen draws it.
@Suite("Team evaluation report")
struct TeamEvaluationReportTests {
    static func sample(from file: String = #filePath) throws -> Evaluation {
        // Tests/AgencyModelTests/Evaluation/TeamEvaluationReportTests.swift: six levels up is the repository root.
        var url = URL(fileURLWithPath: file)
        for _ in 0..<6 { url.deleteLastPathComponent() }
        let data = try Data(contentsOf: url.appendingPathComponent("testdata/evaluation.sample.json"))
        return try JSONDecoder().decode(Evaluation.self, from: data)
    }

    private func report(_ index: Int = 0) throws -> TeamEvaluationReport {
        let evaluation = try Self.sample()
        return TeamEvaluationReport(numbers: evaluation.teams[index], label: index == 0 ? "App" : "Docs", days: evaluation.days)
    }

    @Test("the tiles are the node's numbers, merged counting the reverts, with a median and its p90")
    func tiles() throws {
        let tiles = try report().tiles
        #expect(tiles.map(\.label) == [
            "マージした新機能", "マージしたプルリクエスト", "リードタイム (プルリクエスト)", "リードタイム (issue)",
            "main の CI が失敗していた時間", "リバート", "マージ 1 件あたりのコスト",
        ])
        #expect(tiles[0].value == "3")
        #expect(tiles[0].note == "1 日あたり 0.2")
        // 6 merged by type and 1 revert.
        #expect(tiles[1].value == "7")
        #expect(tiles[1].note == "1 日あたり 0.5")
        #expect(tiles[2].value == "1.5時間")
        #expect(tiles[2].note == "中央値。10 件中 9 件は 4時間 以内。")
        #expect(tiles[3].value == "4.3時間")
        #expect(tiles[4].value == "30分")
        #expect(tiles[4].note == "最長 30分")
        #expect(tiles[5].value == "1")
        #expect(tiles[5].note == "マージ 7 件のうち")
        #expect(tiles[6].value == "$1.90")
    }

    @Test("a team with no pull request merged has no lead time and no cost to state, and says so")
    func emptyTeam() throws {
        let tiles = try report(1).tiles
        #expect(tiles[2].value == "—")
        #expect(tiles[2].note == "この期間はありません")
        #expect(tiles[6].value == "—")
        #expect(tiles[6].note == "この期間はありません")
        #expect(tiles[1].value == "0")
    }

    @Test("five charts, each over every day of the period with none missing")
    func charts() throws {
        let charts = try report().charts
        #expect(charts.map(\.title) == [
            "1 日にマージしたプルリクエスト", "リードタイム", "マージ 1 件あたりのコスト",
            "main の CI が失敗していた時間 (1 日ごと)", "1 日あたりのリバート",
        ])
        #expect(charts.map(\.kind) == [.columns, .lines, .lines, .columns, .columns])
        for chart in charts {
            #expect(chart.days.count == 14)
            #expect(chart.days.first?.date == "2026-11-11")
            #expect(chart.days.last?.date == "2026-11-24")
            #expect(chart.dateLabels == ["11月11日", "11月17日", "11月24日"])
            #expect(chart.axis.count == 3)
        }
    }

    @Test("merged pull requests are stacked by type in a fixed order, new features first, 'other' grey")
    func merged() throws {
        let chart = try report().charts[0]
        #expect(chart.series.map(\.label) == ["新機能", "修正", "リファクタリング", "ドキュメント", "テスト", "雑務", "その他"])
        #expect(chart.series.map(\.slot) == [1, 2, 3, 4, 5, 6, nil])
        let day = try #require(chart.days.first { $0.date == "2026-11-23" })
        #expect(day.values == [2, 1, 0, 0, 0, 0, 0])
        #expect(day.totalText == "3")
        #expect(day.spoken == "11月23日: 3 件マージ")
        // The axis is whole numbers all the way: 3 merged at most.
        #expect(chart.max == 6)
        #expect(chart.axis == ["6", "3", "0"])
    }

    @Test("lead times are a median over the last 7 days, and a day with no sample says so")
    func lead() throws {
        let chart = try report().charts[1]
        #expect(chart.series.map(\.label) == ["プルリクエストを開いてからマージまで", "issue に着手してからマージまで"])
        let first = try #require(chart.days.first)
        #expect(first.values == [nil, nil])
        #expect(first.valueTexts == ["データなし", "データなし"])
        let last = try #require(chart.days.last)
        // 3600, 5400, 14400 and 1800, 7200, 9000, 600 pooled: seven samples, the median is 5400.
        #expect(last.values[0] == 5400)
        #expect(last.valueTexts[0] == "1.5時間")
        #expect(chart.hasNoData == false)
        #expect(chart.axis == ["5時間", "2.5時間", "0時間"])
    }

    @Test("cost per pull request over 7 days; a team that merged nothing has nothing to draw")
    func cost() throws {
        let chart = try report().charts[2]
        let last = try #require(chart.days.last)
        // 13.3 over the 6 pull requests merged and 1 revert in the two days.
        #expect(abs((last.values[0] ?? 0) - 13.3 / 7) < 0.0001)
        let empty = try report(1).charts[2]
        #expect(empty.hasNoData)
        #expect(empty.axis == ["$1.00", "$0.50", "$0"])
    }

    @Test("main's red time per day, and the reverts per day, are single columns that say their value")
    func redAndReverts() throws {
        let red = try report().charts[3]
        #expect(red.series.count == 1)
        let day = try #require(red.days.first { $0.date == "2026-11-23" })
        #expect(day.valueTexts == ["30分"])
        #expect(day.totalText == nil)
        #expect(day.spoken == "11月23日: 30分")
        let reverts = try report().charts[4]
        #expect(reverts.days.first { $0.date == "2026-11-24" }?.valueTexts == ["1"])
    }

    @Test("a team is drawn against its own period, as long as its node says, not the one asked for")
    func ownPeriod() throws {
        let evaluation = try Self.sample()
        let own = TeamEvaluation.with(from: "2026-11-18", until: "2026-11-24", of: evaluation.teams[0])
        let report = TeamEvaluationReport(numbers: own, label: "App", days: 14)
        #expect(report.charts[0].days.count == 7)
        #expect(report.charts[0].days.first?.date == "2026-11-18")
        #expect(report.tiles[0].note == "1 日あたり 0.4")
    }

    @Test("the title names the team by the label the account gives it")
    func title() throws {
        #expect(try report().title == "Appチーム")
    }
}
