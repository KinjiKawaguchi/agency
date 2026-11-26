import AgencyAPI
import Foundation

/// What a team spent by work type, set against what the org intends, ported from `web/src/evaluation/effort.ts`.
public enum EffortAnalysis {
    public static let weekDays = 7

    public static func totalCost(_ effort: TeamEffort) -> Double {
        EffortType.allCases.reduce(0) { $0 + (effort.byType[$1]?.costUsd ?? 0) }
    }

    /// Each type's share of the period's cost, as percentages that add up to 100; all 0 when nothing was spent.
    public static func shares(_ effort: TeamEffort) -> [EffortType: Double] {
        let total = totalCost(effort)
        return EffortType.allCases.reduce(into: [:]) { shares, type in
            shares[type] = total == 0 ? 0 : (effort.byType[type]?.costUsd ?? 0) / total * 100
        }
    }

    /// What a team spent over the seven days that end at `last` (the period's last day), at API prices.
    public static func weekSpend(_ byDay: [DayEffort], last: String) -> Double {
        let days = Set(EvaluationSeries.daysEnding(last, days: weekDays))
        return byDay.filter { days.contains($0.date) }.reduce(0) { sum, day in
            sum + EffortType.allCases.reduce(0) { $0 + (day.costUsd[$1] ?? 0) }
        }
    }

    /// A share as the screen writes it, in whole percent. The bar's figure and the gap to the plan both come from this one rounding.
    public static func shownShare(_ share: Double) -> Int { Int(share.rounded()) }

    /// Percentage points above (+) or below (-) the plan, from the share as shown.
    public static func pointsFromPlan(share: Double, intended: Int) -> Int { shownShare(share) - intended }
}
