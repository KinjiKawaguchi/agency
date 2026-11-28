import AgencyAPI
import Foundation

/// A team's weekly budget as the screen says it: what the node answered, drawn and not judged. The week, the thresholds and the state are the
/// node's (`docs/protocol.md`, "Budget"); the fill's width is spent over the cap for the eye only.
public struct BudgetReport: Equatable, Sendable {
    /// The bar: for a team whose usage was read and that has a cap.
    public struct Meter: Equatable, Sendable {
        /// 0 to 1.
        public let fraction: Double
        public let state: BudgetPart.State
        /// The node's state in words; `nil` for a state this app does not know, which is said as none.
        public let stateText: String?
        public let spoken: String
    }

    /// What was spent of the cap this week; `nil` for a team with no cap, which has no meter of its own.
    public let line: String?
    public let meter: Meter?
    /// Why a team has no spend to show: its usage could not be read.
    public let teamNote: String?
    /// The org's budget, said when it is not `ok` (an org that is over makes its teams over) and when part of it could not be read.
    public let orgNote: String?

    /// Nothing to say: a team with no cap, in an org that is within its budget and was read whole.
    public var isEmpty: Bool { line == nil && teamNote == nil && orgNote == nil }

    public init(_ status: BudgetStatus) {
        let team = status.team
        orgNote = Self.orgNote(status.org)
        guard let cap = team.capUsd else {
            (line, meter, teamNote) = (nil, nil, nil)
            return
        }
        if team.isPartial {
            (line, meter, teamNote) = (Ja.Effort.budgetNotKnown(weekStart: status.weekStart), nil, Ja.Effort.budgetPartial)
            return
        }
        let (spent, limit) = (Self.money(team.spentUsd), Self.money(Double(cap)))
        let words = Self.words(team.state)
        line = Ja.Effort.budgetLine(spent: spent, of: limit, weekStart: status.weekStart)
        teamNote = nil
        meter = Meter(
            fraction: cap == 0 ? (team.state == .over ? 1 : 0) : min(team.spentUsd / Double(cap), 1), state: team.state, stateText: words,
            spoken: Ja.Effort.budgetMeter(spent: spent, of: limit, state: words)
        )
    }

    static func money(_ value: Double) -> String { value == 0 ? "$0" : JaFormat.money(value) }

    private static func words(_ state: BudgetPart.State) -> String? {
        switch state {
        case .ok: Ja.Effort.budgetOK
        case .warning: Ja.Effort.budgetWarning
        case .over: Ja.Effort.budgetOverState
        case .unknown: nil
        }
    }

    private static func orgNote(_ org: BudgetPart) -> String? {
        guard let cap = org.capUsd else { return nil }
        let (spent, limit) = (money(org.spentUsd), money(Double(cap)))
        // A floor of nothing says nothing: when no usage of the org was read at all, it is said that some could not be, with no amount.
        let unread = org.spentUsd == 0
        switch org.state {
        case .ok:
            guard org.isPartial else { return nil }
            return unread ? Ja.Effort.budgetOrgNoneRead : Ja.Effort.budgetOrgPartialOK(spent: spent, of: limit)
        case .warning, .over:
            let said = Ja.Effort.budgetOrg(isOver: org.state == .over, spent: spent, of: limit)
            guard org.isPartial else { return said }
            return said + " " + (unread ? Ja.Effort.budgetOrgNoneRead : Ja.Effort.budgetOrgLowerBound)
        case .unknown:
            return nil
        }
    }
}

/// What is known of one team's weekly budget.
public enum BudgetRead: Equatable, Sendable {
    case loading
    case reported(BudgetReport)
    /// A node that does not report the week's spend: what the plan states, and that the node does not say how the week stands.
    case plain(String)
    /// Nothing to say: the node does not report it and the plan states no budget.
    case none
    case failed(String)
}
