import AgencyAPI
import Foundation

/// What the evaluation screen says of how fast the node answers, worked out of one answer of `GET /api/metrics`: in-node reads
/// against the budget the node names, the objectives the operator set as the node states them, the routes the node answers by
/// group, and the calls it makes. Every percentile is an upper bound ("at most"), and "nothing measured" is its own line, not 0.
public struct NodeSpeed: Equatable, Sendable {
    /// In-node reads against the budget, drawn only when the node names a budget.
    public struct BudgetReading: Equatable, Sendable {
        public let budgetMs: Int
        /// Every in-node read put together; nothing when none was measured.
        public let reads: Latency.Row?
        public let verdict: Latency.Verdict?
        /// The reads that break the budget, one by one.
        public let overRoutes: [Latency.Row]

        public var lead: String { Ja.NodeSpeed.Budget.lead(Ja.NodeSpeed.duration(budgetMs)) }

        /// The verdict in words, or the line that none was measured.
        public var verdictLine: String {
            guard let reads, let p95 = reads.p95, let verdict else { return Ja.NodeSpeed.Budget.noReads }
            let bound = Ja.NodeSpeed.bound(p95)
            switch verdict {
            case .within: return Ja.NodeSpeed.Budget.within(bound)
            case .over: return Ja.NodeSpeed.Budget.over(bound)
            case .undetermined: return Ja.NodeSpeed.Budget.undetermined(bound, Ja.NodeSpeed.duration(budgetMs))
            }
        }

        /// How many reads the verdict is of; nothing when none was measured.
        public var requestsLine: String? {
            guard let reads, reads.p95 != nil, verdict != nil else { return nil }
            return Ja.NodeSpeed.Budget.requests(reads.count)
        }
    }

    public struct ObjectiveLine: Equatable, Identifiable, Sendable {
        public let id: String
        public let target: String
        public let state: Objective.State
        public let measured: String?

        public var stateWord: String {
            switch state {
            case .met: Ja.NodeSpeed.Objectives.met
            case .notMet: Ja.NodeSpeed.Objectives.notMet
            case .unknown: Ja.NodeSpeed.Objectives.unknown
            }
        }
    }

    /// The answer has no series: the node measured and has nothing yet for the period.
    public let hasNoSeries: Bool
    public let budget: BudgetReading?
    public let objectives: [ObjectiveLine]
    /// The routes the node answers, by route group.
    public let routeGroups: [Latency.Group]
    /// The calls the node makes, by group.
    public let callGroups: [Latency.Group]
    /// The routes that are in-node reads, so that a route row can say so.
    public let inNodeReadRoutes: Set<String>

    public init(_ metrics: Metrics) {
        hasNoSeries = metrics.series.isEmpty
        budget = metrics.budget.map { budget in
            let reads = Latency.inNodeReadsRow(metrics.series)
            return BudgetReading(
                budgetMs: budget.inNodeReadP95Ms, reads: reads, verdict: Latency.verdict(of: reads?.p95, budgetMs: budget.inNodeReadP95Ms),
                overRoutes: Latency.readsOverBudget(metrics.series, budgetMs: budget.inNodeReadP95Ms)
            )
        }
        objectives = metrics.objectives.map { objective in
            ObjectiveLine(
                id: objective.group,
                target: Ja.NodeSpeed.Objectives.target(Ja.NodeSpeed.Groups.name(objective.group), Ja.NodeSpeed.bound(.atMost(objective.p95Ms))),
                state: objective.state,
                measured: objective.measuredP95Ms.map { Ja.NodeSpeed.Objectives.measured(Ja.NodeSpeed.bound(.atMost($0))) }
            )
        }
        routeGroups = Latency.groups(of: metrics.series, kind: .serve)
        callGroups = Latency.groups(of: metrics.series, kind: .call)
        inNodeReadRoutes = Set(metrics.series.filter(\.isInNodeRead).map(\.name))
    }
}
