@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

/// Ported from `web/src/evaluation/effort.test.ts`, and the shared sample (`testdata/effort.sample.json`) drawn as the screen draws it.
extension TeamEffort {
    /// A team that spent `spent` dollars on new features on its last day, for a test.
    static func with(spent: Double, last: String, from: String = "2026-11-11", of team: TeamEffort) -> TeamEffort {
        let types = EffortType.allCases.map { #""\#($0.rawValue)":{"costUsd":\#($0 == .newFeature ? spent : 0)}"# }.joined(separator: ",")
        let json = #"{"team":"\#(team.team)","from":"\#(from)","until":"\#(last)","byType":{\#(types)},"byDay":[{"date":"\#(last)","costUsd":{"newFeature":\#(spent)}}]}"#
        return try! JSONDecoder().decode(TeamEffort.self, from: Data(json.utf8))
    }
}

@Suite("Effort")
struct EffortReportTests {
    static func sample(from file: String = #filePath) throws -> Effort {
        var url = URL(fileURLWithPath: file)
        for _ in 0..<6 { url.deleteLastPathComponent() }
        let data = try Data(contentsOf: url.appendingPathComponent("testdata/effort.sample.json"))
        return try JSONDecoder().decode(Effort.self, from: data)
    }

    private func effort(_ costs: [EffortType: Double] = [:], byDay: [DayEffort] = [], until: String = "2026-11-24") -> TeamEffort {
        let types = EffortType.allCases.map { #""\#($0.rawValue)":{"costUsd":\#(costs[$0] ?? 0)}"# }.joined(separator: ",")
        let days = byDay.map { day in
            let own = EffortType.allCases.map { #""\#($0.rawValue)":\#(day.costUsd[$0] ?? 0)"# }.joined(separator: ",")
            return #"{"date":"\#(day.date)","costUsd":{\#(own)}}"#
        }.joined(separator: ",")
        let json = #"{"team":"app","from":"2026-11-11","until":"\#(until)","byType":{\#(types)},"byDay":[\#(days)]}"#
        return try! JSONDecoder().decode(TeamEffort.self, from: Data(json.utf8))
    }

    private let plan = TeamPlan(allocation: [.newFeature: 50, .improvement: 20, .maintenance: 20, .debt: 10, .incident: 0], weeklyBudgetUsd: 140)

    @Test("shares divide each type's cost by the whole, the unclassified work included")
    func shares() {
        let shares = EffortAnalysis.shares(effort([.newFeature: 30, .maintenance: 50, .unclassified: 20]))
        #expect(shares[.newFeature] == 30)
        #expect(shares[.improvement] == 0)
        #expect(shares[.maintenance] == 50)
        #expect(shares[.unclassified] == 20)
        #expect(EffortAnalysis.totalCost(effort([.newFeature: 30, .unclassified: 20])) == 50)
    }

    @Test("shares are all zero, not not-a-number, when nothing was spent")
    func nothingSpent() {
        #expect(EffortAnalysis.shares(effort()).values.allSatisfy { $0 == 0 })
    }

    @Test("the gap to the plan is measured in whole points, from the share as shown")
    func points() {
        #expect(EffortAnalysis.pointsFromPlan(share: 43.4, intended: 50) == -7)
        #expect(EffortAnalysis.pointsFromPlan(share: 20.4, intended: 20) == 0)
        #expect(EffortAnalysis.pointsFromPlan(share: 20.5, intended: 20) == 1)
    }

    @Test("the week's spend adds up the seven days that end at the last day of the period, and no earlier day")
    func weekSpend() {
        let day = { (date: String, feature: Double, unclassified: Double) in DayEffort(date: date, costUsd: [.newFeature: feature, .unclassified: unclassified]) }
        let byDay = [day("2026-11-19", 100, 0), day("2026-11-20", 1, 0), day("2026-11-24", 2, 1), day("2026-11-26", 3, 0)]
        #expect(EffortAnalysis.weekSpend(byDay, last: "2026-11-26") == 7)
    }

    // MARK: The report

    @Test("the sample against a plan: each type's share and the org's mark, the gap in points")
    func allocation() throws {
        let sample = try Self.sample()
        let report = TeamEffortReport(effort: sample.teams[0], label: "App", days: 14, plan: .stated(plan))
        #expect(report.rows.map(\.label) == ["新機能", "改善", "保守", "技術的負債", "障害対応", "未分類"])
        #expect(report.rows.map(\.slot) == [1, 2, 3, 4, 5, nil])
        // 8.1 of 13.3 is 60.9%; the plan says 50.
        #expect(report.rows[0].shareText == "コストの 61%")
        #expect(report.rows[0].intendedText == "意図 50%、11 ポイント上")
        #expect(report.rows[1].shareText == "コストの 0%")
        #expect(report.rows[1].intendedText == "意図 20%、20 ポイント下")
        #expect(report.rows[5].intended == nil)
        #expect(report.rows[5].intendedText == nil)
        #expect(report.planNote == nil)
    }

    @Test("a team on plan says so")
    func onPlan() throws {
        let sample = try Self.sample()
        let exact = TeamPlan(allocation: [.newFeature: 61, .improvement: 0, .maintenance: 24, .debt: 11, .incident: 4], weeklyBudgetUsd: 10)
        let report = TeamEffortReport(effort: sample.teams[0], label: "App", days: 14, plan: .stated(exact))
        #expect(report.rows[0].intendedText == "意図 61%、プランどおり")
    }

    @Test("without a plan nothing is compared, and the screen says why; an unreadable plan says that")
    func noPlan() throws {
        let sample = try Self.sample()
        let none = TeamEffortReport(effort: sample.teams[0], label: "App", days: 14, plan: .none)
        #expect(none.planNote == "このチームには組織のプランがまだ示されていないので、比べるものがありません。")
        #expect(none.rows.allSatisfy { $0.intendedText == nil })
        #expect(none.budget == nil)
        let unreadable = TeamEffortReport(effort: sample.teams[0], label: "App", days: 14, plan: .unreadable)
        #expect(unreadable.planNote == "このチームの組織のプランを読み取れなかったので、比べていません。")
    }

    @Test("the weekly budget: what was spent in the last seven days, what is left, or by how much it is over")
    func budget() throws {
        let sample = try Self.sample()
        // The sample's two days cost 13.3 in all.
        let report = TeamEffortReport(effort: sample.teams[0], label: "App", days: 14, plan: .stated(plan))
        let budget = try #require(report.budget)
        #expect(budget.line == "直近 7 日で $140 のうち $13")
        #expect(budget.note == "残り $127")
        #expect(budget.isOver == false)
        #expect(abs(budget.fraction - 13.3 / 140) < 0.0001)
        #expect(budget.spoken == "週次予算 $140 のうち $13 を使用")
        let tight = TeamPlan(allocation: plan.allocation, weeklyBudgetUsd: 10)
        let over = try #require(TeamEffortReport(effort: sample.teams[0], label: "App", days: 14, plan: .stated(tight)).budget)
        #expect(over.isOver)
        #expect(over.note == "$3.30 超過")
        #expect(over.fraction == 1)
        let none = TeamPlan(allocation: plan.allocation, weeklyBudgetUsd: 0)
        #expect(try #require(TeamEffortReport(effort: sample.teams[0], label: "App", days: 14, plan: .stated(none)).budget).fraction == 1)
    }

    @Test("spending exactly the budget is not over it")
    func exactBudget() throws {
        let sample = try Self.sample()
        let exact = TeamPlan(allocation: plan.allocation, weeklyBudgetUsd: 13)
        let day = TeamEffort.with(spent: 13, last: "2026-11-24", of: sample.teams[0])
        let budget = try #require(TeamEffortReport(effort: day, label: "App", days: 14, plan: .stated(exact)).budget)
        #expect(!budget.isOver)
        #expect(budget.note == "残り $0")
        #expect(budget.fraction == 1)
    }

    @Test("a team is drawn against its own period, not the one asked for")
    func ownPeriod() throws {
        let sample = try Self.sample()
        let short = TeamEffort.with(spent: 1, last: "2026-11-24", from: "2026-11-18", of: sample.teams[0])
        let chart = TeamEffortReport(effort: short, label: "App", days: 14, plan: .none).byDay
        #expect(chart.days.count == 7)
        #expect(chart.days.first?.date == "2026-11-18")
    }

    @Test("the cost by day is stacked by type over every day of the period, with the total on the day")
    func byDay() throws {
        let sample = try Self.sample()
        let chart = TeamEffortReport(effort: sample.teams[0], label: "App", days: 14, plan: .none).byDay
        #expect(chart.days.count == 14)
        #expect(chart.series.map(\.label) == ["新機能", "改善", "保守", "技術的負債", "障害対応", "未分類"])
        let day = try #require(chart.days.first { $0.date == "2026-11-23" })
        #expect(day.values == [5.1, 0, 2.0, 1.5, 0.5, 0])
        #expect(day.totalText == "$9.10")
        #expect(day.spoken == "11月23日: $9.10")
        #expect(chart.days.first?.values == [0, 0, 0, 0, 0, 0])
        #expect(chart.max == 10)
        #expect(chart.axis == ["$10", "$5.00", "$0"])
    }

    @Test("the table lists each type's cost, share and the plan's share with the gap, and the total")
    func table() throws {
        let sample = try Self.sample()
        let table = TeamEffortReport(effort: sample.teams[0], label: "App", days: 14, plan: .stated(plan)).table
        #expect(table.count == 7)
        #expect(table[0] == TeamEffortReport.TableRow(label: "新機能", cost: "$8.10", share: "61%", intended: "50% (+11)"))
        #expect(table[5].intended == "—")
        #expect(table[6] == TeamEffortReport.TableRow(label: "合計", cost: "$13", share: "", intended: ""))
    }

    @Test("a team that spent nothing has empty bars and an axis")
    func spentNothing() {
        let report = TeamEffortReport(effort: effort(), label: "Quiet", days: 14, plan: .stated(plan))
        #expect(report.rows.allSatisfy { $0.share == 0 })
        #expect(report.byDay.max == 1)
        #expect(report.budget?.line == "直近 7 日で $140 のうち $0")
        #expect(report.budget?.fraction == 0)
        // A budget of nothing is full, even before a dollar is spent.
        let zero = TeamEffortReport(effort: effort(), label: "Quiet", days: 14, plan: .stated(TeamPlan(allocation: plan.allocation, weeklyBudgetUsd: 0)))
        #expect(zero.budget?.fraction == 1)
    }
}
