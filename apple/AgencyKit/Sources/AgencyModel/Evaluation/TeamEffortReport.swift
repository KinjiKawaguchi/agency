import AgencyAPI
import Foundation

/// What the org's plan says of a team: stated, not stated, or stated in a shape that cannot be trusted.
public enum PlanState: Equatable, Sendable {
    case stated(TeamPlan)
    case none
    case unreadable
}

/// A team's effort as the screen shows it: its cost by work type against the plan, and its cost by day.
public struct TeamEffortReport: Equatable, Sendable, Identifiable {
    /// One work type: its share of the period's cost, and the org's intended share beside it.
    public struct AllocationRow: Equatable, Sendable, Identifiable {
        public let type: EffortType
        public let label: String
        /// The color slot, 1 to 5; `nil` for the grey of the unclassified work.
        public let slot: Int?
        /// 0 to 100.
        public let share: Double
        public let shareText: String
        /// 0 to 100, when the plan states one for this type.
        public let intended: Int?
        /// `意図 50%, 7 ポイント下`, when the plan states one.
        public let intendedText: String?
        public var id: EffortType { type }
    }

    public struct TableRow: Equatable, Sendable, Identifiable {
        public let label: String
        public let cost: String
        public let share: String
        public let intended: String
        public var id: String { label }
    }

    public let team: String
    public let label: String
    public let rows: [AllocationRow]
    /// Why there is nothing to compare, when there is not.
    public let planNote: String?
    /// The weekly budget the plan states, in whole US dollars: what the plan intends, not how the week stands (that is the node's, `BudgetReport`).
    public let plannedBudget: Int?
    public let byDay: EvaluationChart
    public let table: [TableRow]
    public var id: String { team }
    public var title: String { Ja.Evaluation.team(label) }

    private static func money(_ value: Double) -> String { value == 0 ? "$0" : JaFormat.money(value) }

    /// - Parameter days: the period the person asked for, used when the team's own `from` and `until` cannot be read.
    public init(effort: TeamEffort, label: String, days asked: Int, plan state: PlanState) {
        let days = EvaluationSeries.dayCount(from: effort.from, until: effort.until) ?? asked
        let plan: TeamPlan? = if case .stated(let stated) = state { stated } else { nil }
        let shares = EffortAnalysis.shares(effort)
        team = effort.team
        self.label = label
        planNote = switch state {
        case .stated: nil
        case .none: Ja.Effort.noPlan
        case .unreadable: Ja.Effort.planUnreadable
        }
        rows = EffortType.allCases.map { type in Self.row(type, share: shares[type] ?? 0, intended: plan?.allocation[type]) }
        plannedBudget = plan?.weeklyBudgetUsd
        byDay = Self.chart(effort, days: days)
        table = Self.table(effort, shares: shares, plan: plan)
    }

    // MARK: Parts

    private static func slot(_ type: EffortType) -> Int? {
        EffortType.allCases.firstIndex(of: type).flatMap { type == .unclassified ? nil : $0 + 1 }
    }

    private static func row(_ type: EffortType, share: Double, intended: Int?) -> AllocationRow {
        let intendedText = intended.map { intended -> String in
            let points = EffortAnalysis.pointsFromPlan(share: share, intended: intended)
            let gap = points == 0 ? Ja.Effort.onPlan : points > 0 ? Ja.Effort.above(points) : Ja.Effort.below(-points)
            return "\(Ja.Effort.intended(JaFormat.percent(Double(intended))))、\(gap)"
        }
        return AllocationRow(
            type: type, label: Ja.Effort.type(type), slot: slot(type), share: min(share, 100),
            shareText: Ja.Effort.actual(JaFormat.percent(Double(EffortAnalysis.shownShare(share)))),
            intended: intended, intendedText: intendedText
        )
    }

    private static func chart(_ effort: TeamEffort, days: Int) -> EvaluationChart {
        let dates = EvaluationSeries.daysEnding(effort.until, days: days)
        let known = Dictionary(effort.byDay.map { ($0.date, $0) }, uniquingKeysWith: { first, _ in first })
        let values = dates.map { date in EffortType.allCases.map { known[date]?.costUsd[$0] ?? 0 } }
        let maximum = EvaluationSeries.niceMax(values.map { $0.reduce(0, +) }.max() ?? 0)
        return ChartBuilder.columns(
            title: Ja.Effort.byDay, hint: Ja.Effort.byDayHint,
            series: EffortType.allCases.map { ChartSeries(key: $0.rawValue, label: Ja.Effort.type($0), slot: slot($0)) },
            dates: dates, values: values, max: maximum, axis: [maximum, maximum / 2, 0].map(money), text: money, hasTotal: true,
            spoken: { "\($0): \(money($1))" }
        )
    }

    private static func table(_ effort: TeamEffort, shares: [EffortType: Double], plan: TeamPlan?) -> [TableRow] {
        let rows = EffortType.allCases.map { type -> TableRow in
            let share = shares[type] ?? 0
            let intended = plan?.allocation[type].map { intended -> String in
                let points = EffortAnalysis.pointsFromPlan(share: share, intended: intended)
                return "\(JaFormat.percent(Double(intended))) (\(points > 0 ? "+" : "")\(points))"
            }
            return TableRow(
                label: Ja.Effort.type(type), cost: money(effort.byType[type]?.costUsd ?? 0),
                share: JaFormat.percent(Double(EffortAnalysis.shownShare(share))), intended: intended ?? "—"
            )
        }
        return rows + [TableRow(label: Ja.Effort.tableTotal, cost: money(EffortAnalysis.totalCost(effort)), share: "", intended: "")]
    }
}
