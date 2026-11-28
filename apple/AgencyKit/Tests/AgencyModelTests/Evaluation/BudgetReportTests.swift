import AgencyAPI
import Testing
@testable import AgencyModel

@Suite("Saying the weekly budget")
struct BudgetReportTests {
    private let week = "2026-11-23"
    private func report(_ team: BudgetPart, org: BudgetPart = BudgetPart(capUsd: 500, spentUsd: 100, state: .ok)) -> BudgetReport {
        BudgetReport(BudgetStatus(weekStart: week, team: team, org: org))
    }

    @Test("what was spent of the cap this week, from Monday, with the node's state in words and the fill as spent over the cap")
    func states() throws {
        let ok = try #require(report(BudgetPart(capUsd: 200, spentUsd: 40, state: .ok)).meter)
        #expect(ok.stateText == "予算の範囲内です" && ok.state == .ok && ok.fraction == 0.2)
        #expect(ok.spoken == "週次予算 $200 のうち $40 を使用: 予算の範囲内です")
        #expect(report(BudgetPart(capUsd: 200, spentUsd: 40, state: .ok)).line == "今週(2026-11-23 の月曜から)は $200 のうち $40")
        let warning = try #require(report(BudgetPart(capUsd: 100, spentUsd: 86.4, state: .warning)).meter)
        #expect(warning.stateText == "予算に近づいています" && warning.state == .warning)
        let over = try #require(report(BudgetPart(capUsd: 100, spentUsd: 120, state: .over)).meter)
        #expect(over.stateText == "予算を超えています" && over.fraction == 1, "the fill stops at the end of the bar")
    }

    @Test("the state is the node's word: a spend over the cap that the node calls ok is said as ok, and nothing is compared here")
    func nothingIsCompared() throws {
        let said = try #require(report(BudgetPart(capUsd: 100, spentUsd: 120, state: .ok)).meter)
        #expect(said.state == .ok && said.stateText == "予算の範囲内です")
        let unknown = try #require(report(BudgetPart(capUsd: 100, spentUsd: 120, state: .unknown)).meter)
        #expect(unknown.stateText == nil, "a state the app does not know is said as none")
        #expect(unknown.spoken == "週次予算 $100 のうち $120 を使用")
    }

    @Test("a cap of nothing: the fill is empty, or full when the node says over")
    func zeroCap() throws {
        #expect(try #require(report(BudgetPart(capUsd: 0, spentUsd: 0, state: .ok)).meter).fraction == 0)
        #expect(try #require(report(BudgetPart(capUsd: 0, spentUsd: 3, state: .over)).meter).fraction == 1)
    }

    @Test("a team whose usage could not be read is not known, never 0, with no bar and no state")
    func teamPartial() {
        let said = report(BudgetPart(capUsd: 200, spentUsd: 0, state: .ok, isPartial: true))
        #expect(said.meter == nil)
        #expect(said.line == "今週(2026-11-23 の月曜から)の使用額は分かりません")
        #expect(said.teamNote == "このチームの使用量を読めなかったため、今週の使用額は分かりません。")
        #expect(!(said.line ?? "").contains("$0"))
    }

    @Test("a team with no cap has no meter of its own, and says nothing when its org is within the budget and was read whole")
    func noCap() {
        let said = report(BudgetPart(capUsd: nil, spentUsd: 5, state: .ok))
        #expect(said.line == nil && said.meter == nil && said.teamNote == nil && said.orgNote == nil)
        #expect(said.isEmpty)
    }

    @Test("the org's budget is said when it is not ok, since an org that is over makes its teams over, and a lower bound is said as one")
    func org() {
        let team = BudgetPart(capUsd: 200, spentUsd: 10, state: .over)
        #expect(report(team, org: BudgetPart(capUsd: 500, spentUsd: 600, state: .over)).orgNote == "組織は週次予算を超えています: $500 のうち $600。")
        #expect(report(team, org: BudgetPart(capUsd: 500, spentUsd: 420, state: .warning)).orgNote == "組織は週次予算に近づいています: $500 のうち $420。")
        #expect(report(team, org: BudgetPart(capUsd: 500, spentUsd: 600, state: .over, isPartial: true)).orgNote
            == "組織は週次予算を超えています: $500 のうち $600。 読めなかった使用量があるため、少なくともこれだけです。")
        #expect(report(team, org: BudgetPart(capUsd: 500, spentUsd: 311, state: .ok, isPartial: true)).orgNote
            == "組織の使用量に読めなかったものがあります: 今週は $500 のうち、少なくとも $311。")
        #expect(report(team, org: BudgetPart(capUsd: 500, spentUsd: 311, state: .ok)).orgNote == nil)
        #expect(report(team, org: BudgetPart(capUsd: nil, spentUsd: 311, state: .over)).orgNote == nil, "an org with no cap says no budget")
    }

    @Test("a floor of nothing says nothing: when no usage of the org was read, it is said that some could not be, with no amount")
    func orgNoneRead() {
        let team = BudgetPart(capUsd: 200, spentUsd: 10, state: .ok)
        let none = "組織の使用量に読めなかったものがあるため、今週の使用額は分かりません。"
        #expect(report(team, org: BudgetPart(capUsd: 500, spentUsd: 0, state: .ok, isPartial: true)).orgNote == none)
        #expect(report(team, org: BudgetPart(capUsd: 500, spentUsd: 0, state: .warning, isPartial: true)).orgNote?.hasSuffix(none) == true)
        #expect(!(report(team, org: BudgetPart(capUsd: 500, spentUsd: 0, state: .ok, isPartial: true)).orgNote ?? "").contains("$"))
    }

    @Test("a state of the org that the app does not know is not said")
    func orgUnknown() {
        #expect(report(BudgetPart(capUsd: 1, spentUsd: 0, state: .ok), org: BudgetPart(capUsd: 5, spentUsd: 1, state: .unknown)).orgNote == nil)
    }
}
