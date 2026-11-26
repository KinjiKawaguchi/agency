#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// The node's own response times, at the end of the evaluation screen, for the owner. A node that says the viewer is not its owner
/// has nothing drawn here at all.
struct NodeSpeedSection: View {
    let store: NodeSpeedStore

    var body: some View {
        if !store.isHidden {
            NodeSpeedContent(
                days: store.days, answer: store.answer,
                onChooseDays: { days in Task { await store.choose(days: days) } },
                onTryAgain: { Task { await store.refresh() } }
            )
        }
    }
}

/// The section's content, without the store: nothing but a skeleton before the node has answered the period, one line when the
/// node does not measure or has measured nothing yet (which is not 0 ms), and every percentile written "at most".
struct NodeSpeedContent: View {
    let days: Int
    let answer: NodeSpeedAnswer?
    var onChooseDays: ((Int) -> Void)?
    var onTryAgain: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            Text(Ja.NodeSpeed.title).font(.title3.weight(.semibold)).wrapping()
            Text(Ja.NodeSpeed.lead).font(.subheadline).foregroundStyle(.secondary).wrapping()
            Text(Ja.NodeSpeed.period).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary)
            HStack(spacing: 8) {
                ForEach(NodeSpeedStore.periods, id: \.self) { period in
                    Button(Ja.NodeSpeed.days(period)) { onChooseDays?(period) }
                        .buttonStyle(ChoiceStyle(isSelected: period == days))
                }
            }
            result
        }
        .padding(.top, 20)
    }

    @ViewBuilder
    private var result: some View {
        switch answer {
        case nil:
            SkeletonCard().accessibilityLabel(Ja.NodeSpeed.loading)
        case .unavailable?:
            Text(Ja.NodeSpeed.unavailable).font(.subheadline).foregroundStyle(.secondary).wrapping()
        case .couldNotAnswer(let reason)?, .failed(let reason)?:
            FailedRegion(what: Ja.NodeSpeed.title, why: reason, onTryAgain: onTryAgain)
        case .ready(let speed)?:
            NodeSpeedReady(speed: speed)
        }
    }
}

private struct NodeSpeedReady: View {
    let speed: NodeSpeed

    var body: some View {
        if speed.hasNoSeries {
            Text(Ja.NodeSpeed.none).font(.subheadline).foregroundStyle(.secondary).wrapping()
        } else {
            VStack(alignment: .leading, spacing: Theme.spacing) {
                if let budget = speed.budget { BudgetCard(budget: budget) }
                if !speed.objectives.isEmpty { ObjectivesCard(lines: speed.objectives) }
                GroupsCard(title: Ja.NodeSpeed.Groups.title, groups: speed.routeGroups, inNodeReadRoutes: speed.inNodeReadRoutes)
                GroupsCard(title: Ja.NodeSpeed.Groups.callsTitle, groups: speed.callGroups, inNodeReadRoutes: [])
            }
        }
    }
}

private struct BudgetCard: View {
    let budget: NodeSpeed.BudgetReading

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(Ja.NodeSpeed.Budget.title).font(.headline).wrapping()
            Text(budget.lead).font(.footnote).foregroundStyle(.secondary).wrapping()
            Label {
                Text(budget.verdictLine).font(.subheadline.weight(.semibold)).wrapping()
            } icon: {
                Image(systemName: symbol).foregroundStyle(color)
            }
            if let requests = budget.requestsLine { Text(requests).font(.footnote).foregroundStyle(.secondary).wrapping() }
            if !budget.overRoutes.isEmpty {
                Text(Ja.NodeSpeed.Budget.overRoutes).font(.subheadline.weight(.semibold)).padding(.top, 4)
                ForEach(budget.overRoutes) { row in
                    VStack(alignment: .leading, spacing: 2) {
                        Text(row.key).font(.system(.footnote, design: .monospaced)).wrapping()
                        if let p95 = row.p95 { Text(Ja.NodeSpeed.bound(p95)).font(.footnote).foregroundStyle(.secondary) }
                    }
                }
            }
        }
        .card()
        .accessibilityElement(children: .combine)
    }

    private var symbol: String {
        switch budget.verdict {
        case .within?: "checkmark.circle.fill"
        case .over?: "exclamationmark.triangle.fill"
        case nil: "questionmark.circle"
        }
    }

    private var color: Color {
        switch budget.verdict {
        case .within?: .green
        case .over?: Theme.danger
        case nil: .secondary
        }
    }
}

private struct ObjectivesCard: View {
    let lines: [NodeSpeed.ObjectiveLine]

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(Ja.NodeSpeed.Objectives.title).font(.headline)
            Text(Ja.NodeSpeed.Objectives.lead).font(.footnote).foregroundStyle(.secondary).wrapping()
            ForEach(lines) { line in
                VStack(alignment: .leading, spacing: 2) {
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Image(systemName: symbol(line.state)).foregroundStyle(color(line.state)).accessibilityHidden(true)
                        Text(line.target).font(.subheadline).wrapping()
                    }
                    Text(line.stateWord).font(.subheadline.weight(.semibold))
                    if let measured = line.measured { Text(measured).font(.footnote).foregroundStyle(.secondary).wrapping() }
                }
                .accessibilityElement(children: .combine)
            }
        }
        .card()
    }

    private func symbol(_ state: Objective.State) -> String {
        switch state {
        case .met: "checkmark.circle.fill"
        case .notMet: "xmark.circle.fill"
        case .unknown: "questionmark.circle"
        }
    }

    private func color(_ state: Objective.State) -> Color {
        switch state {
        case .met: .green
        case .notMet: Theme.danger
        case .unknown: .secondary
        }
    }
}

/// What a group or a route says of its calls: the numbers, each under its label, wrapping to the width.
private struct NumbersGrid: View {
    let row: Latency.Row

    var body: some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 96), alignment: .leading)], alignment: .leading, spacing: 6) {
            cell(Ja.NodeSpeed.Groups.requests, JaFormat.integer(row.count))
            cell(Ja.NodeSpeed.Groups.errors, JaFormat.integer(row.errors))
            // A row with no duration has no percentile to give: its cell says nothing, and is not a dash that stands for a number.
            if let p50 = row.p50 { cell("p50", Ja.NodeSpeed.bound(p50)) }
            if let p95 = row.p95 { cell("p95", Ja.NodeSpeed.bound(p95)) }
            if let p99 = row.p99 { cell("p99", Ja.NodeSpeed.bound(p99)) }
        }
    }

    private func cell(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(label).font(.caption2).foregroundStyle(.secondary)
            Text(value).font(.footnote.monospacedDigit()).wrapping()
        }
        .accessibilityElement(children: .combine)
    }
}

private struct GroupsCard: View {
    let title: String
    let groups: [Latency.Group]
    let inNodeReadRoutes: Set<String>

    var body: some View {
        // Nothing is drawn for a kind with no measured call.
        if !groups.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Text(title).font(.headline).wrapping()
                ForEach(groups) { group in
                    DisclosureGroup {
                        ForEach(group.routes) { route in
                            VStack(alignment: .leading, spacing: 4) {
                                Text(route.key).font(.system(.footnote, design: .monospaced)).wrapping()
                                if inNodeReadRoutes.contains(route.key) {
                                    Text(Ja.NodeSpeed.Groups.inNodeRead).font(.caption2.weight(.semibold)).foregroundStyle(.secondary)
                                }
                                NumbersGrid(row: route)
                            }
                            .padding(.vertical, 4)
                        }
                    } label: {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(Ja.NodeSpeed.Groups.name(group.row.key)).font(.subheadline.weight(.semibold)).wrapping()
                            NumbersGrid(row: group.row)
                        }
                    }
                }
            }
            .card()
        }
    }
}

#endif
