import AgencyAPI
import Foundation

/// What the org's work cost on one day a KPI is drawn over.
public struct CostDay: Equatable, Sendable {
    public let date: String
    /// The API-priced cost of the day over the teams whose period covers it; `nil` when no team's period does, which is not a
    /// cost of 0.
    public let usd: Double?
}

/// What the org's work cost, API-priced, on the days a KPI is drawn over, so that a movement can be read next to the effort of
/// its days: the sum of what each team reports for the day (`TeamEvaluation.byDay`).
///
/// A team reports a day when it lies inside the team's own period (`from` to `until`, in its own node's time zone). The node
/// leaves out of `byDay` a day with nothing in it, so a day inside the period that `byDay` omits is a day of 0; a day inside no
/// team's period has no cost, which is not a cost of 0. A team that did not answer at all is counted, so that the screen can say
/// the sum is of the others.
public struct CostOnDays: Equatable, Sendable {
    public let days: [CostDay]
    public let teamsWithout: Int

    public init(evaluation: Evaluation, dates: [String]) {
        days = dates.map { date in
            let covering = evaluation.teams.filter { $0.from <= date && date <= $0.until }
            guard !covering.isEmpty else { return CostDay(date: date, usd: nil) }
            // A day of the team's period that `byDay` leaves out had nothing in it: it cost 0.
            let total = covering.reduce(0.0) { sum, team in sum + (team.byDay.first { $0.date == date }?.costUsd ?? 0) }
            return CostDay(date: date, usd: total)
        }
        teamsWithout = evaluation.unavailable.count
    }

    /// The days that have a cost.
    public var known: [CostDay] { days.filter { $0.usd != nil } }

    /// The top of the axis: nice over the dearest day.
    public var maximum: Double { EvaluationSeries.niceMax(known.compactMap(\.usd).max() ?? 0) }

    /// What a day says of itself, for a tap or a screen reader.
    public func describe(_ day: CostDay) -> String {
        day.usd.map { Ja.Outcomes.Cost.day(day.date, JaFormat.money($0)) } ?? Ja.Outcomes.Cost.day(day.date, Ja.Outcomes.Cost.noCost)
    }

    /// Said under the columns when some team did not answer: the sum is of the others.
    public var partialLine: String? { teamsWithout == 0 ? nil : Ja.Outcomes.Cost.partial(teamsWithout) }
}

/// What is known of the cost under the KPIs of one org.
public enum OrgCost: Equatable, Sendable {
    case ready(CostOnDays)
    /// The cost could not be read (the node does not answer for it, or a team's runtime did not).
    case unavailable
}

extension OutcomeDay {
    /// Every calendar day from `from` to `until`, both included, as `YYYY-MM-DD`; none when either is not a day or `until` is
    /// before `from`. At most a year.
    static func dates(from: String, until: String) -> [String] {
        guard let first = number(from), let last = number(until), last >= first, last - first < 366 else { return [] }
        return (first...last).map(iso)
    }

    private static func iso(_ number: Int) -> String {
        Date(timeIntervalSince1970: Double(number) * 86_400).formatted(Date.ISO8601FormatStyle(timeZone: .gmt).year().month().day())
    }
}
