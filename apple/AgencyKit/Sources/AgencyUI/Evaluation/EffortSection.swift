#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// Where each team's effort goes, by work type, against the plan in the org definition.
struct EffortSection: View {
    let state: EffortState
    /// How each team's week stands against its weekly budget, by team, as the node said.
    var budgets: [String: BudgetRead] = [:]
    var onTryAgain: (() -> Void)?
    var selectedDay: String?

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            Text(Ja.Effort.title).font(.title3.weight(.semibold)).wrapping()
            Text(Ja.Effort.lead).font(.subheadline).foregroundStyle(.secondary).wrapping()
            switch state {
            case .loading:
                SkeletonCard()
            case .unavailable:
                Text(Ja.Effort.unavailable).font(.subheadline).foregroundStyle(.secondary).wrapping()
            case .couldNotAnswer(let reason), .failed(let reason):
                FailedRegion(what: Ja.Effort.title, why: reason, onTryAgain: onTryAgain)
            case .ready(let reports, let unavailable):
                ForEach(reports) { TeamEffortView(report: $0, budget: budgets[$0.team], selectedDay: selectedDay).padding(.top, 8) }
                ForEach(unavailable) { note in
                    VStack(alignment: .leading, spacing: 6) {
                        Text(note.title).font(.headline).wrapping()
                        Text(note.text).font(.subheadline).foregroundStyle(.secondary).wrapping()
                    }
                    .padding(.top, 8)
                }
            }
        }
    }
}

/// One team's effort: its cost by work type against the plan, its weekly budget, and its cost by day.
struct TeamEffortView: View {
    let report: TeamEffortReport
    /// What the node said of the team's week; `nil` before it has been asked.
    var budget: BudgetRead?
    var selectedDay: String?

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            Text(report.title).font(.headline).wrapping()
            allocation
            EvaluationChartView(chart: report.byDay, selectedDay: selectedDay)
            EffortTable(rows: report.table)
        }
    }

    private var allocation: some View {
        VStack(alignment: .leading, spacing: 10) {
            VStack(alignment: .leading, spacing: 2) {
                Text(Ja.Effort.allocationTitle).font(.subheadline.weight(.semibold)).wrapping()
                if report.planNote == nil { Text(Ja.Effort.allocationHint).font(.caption).foregroundStyle(.secondary).wrapping() }
            }
            if let note = report.planNote { Text(note).font(.footnote).foregroundStyle(.secondary).wrapping() }
            ForEach(report.rows) { AllocationRowView(row: $0) }
            Text(Ja.Effort.unclassifiedNote).font(.caption).foregroundStyle(.secondary).wrapping()
            if let budget { WeeklyBudgetView(read: budget) }
        }
        .card()
    }
}

/// A type's share of the cost as a bar on a 0 to 100% track, and the org's intended share as a mark on the same
/// track, so how far off a type is reads as the gap between them.
private struct AllocationRowView: View {
    let row: TeamEffortReport.AllocationRow

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 6) {
                Circle().fill(SeriesColor.slot(row.slot)).frame(width: 8, height: 8)
                Text(row.label).font(.subheadline).wrapping()
            }
            GeometryReader { proxy in
                ZStack(alignment: .leading) {
                    Capsule().fill(Theme.hairline)
                    Capsule().fill(SeriesColor.slot(row.slot)).frame(width: proxy.size.width * row.share / 100)
                    if let intended = row.intended {
                        Rectangle().fill(Color.primary).frame(width: 2, height: 14)
                            .offset(x: min(max(proxy.size.width * CGFloat(intended) / 100 - 1, 0), proxy.size.width - 2))
                    }
                }
            }
            .frame(height: 10)
            .accessibilityHidden(true)
            Text(row.shareText).font(.footnote.weight(.semibold))
            if let intended = row.intendedText { Text(intended).font(.caption).foregroundStyle(.secondary).wrapping() }
        }
        .accessibilityElement(children: .combine)
    }
}

/// A team's weekly budget as the node says it: this week from Monday, what was spent of the cap, and the node's own state. Nothing is compared
/// here: the fill's width is spent over the cap for the eye, and its color and the sentence are the state's. A team whose usage could not be
/// read has no spend and no state, and says so with no bar.
struct WeeklyBudgetView: View {
    let read: BudgetRead

    var body: some View {
        switch read {
        case .loading:
            SkeletonCard().padding(.top, 6)
        case .none:
            EmptyView()
        case .plain(let note):
            Text(note).font(.footnote).foregroundStyle(.secondary).wrapping().padding(.top, 6)
        case .failed(let reason):
            Text(Ja.Effort.budgetFailed(reason)).font(.footnote).foregroundStyle(Theme.danger).wrapping().padding(.top, 6)
        case .reported(let report):
            if !report.isEmpty { reported(report) }
        }
    }

    private func reported(_ report: BudgetReport) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            if let line = report.line {
                Text(Ja.Effort.budgetTitle).font(.subheadline.weight(.semibold)).wrapping()
                Text(line).font(.caption).foregroundStyle(.secondary).wrapping()
            }
            if let meter = report.meter { Meter(meter: meter) }
            if let note = report.teamNote { Text(note).font(.footnote).foregroundStyle(.secondary).wrapping() }
            if let note = report.orgNote { Text(note).font(.footnote).foregroundStyle(.secondary).wrapping() }
        }
        .padding(.top, 6)
    }

    private struct Meter: View {
        let meter: BudgetReport.Meter

        private var fill: Color {
            switch meter.state {
            case .over: Theme.danger
            case .warning: Theme.warning
            case .ok, .unknown: Color.accentColor
            }
        }

        var body: some View {
            VStack(alignment: .leading, spacing: 4) {
                GeometryReader { proxy in
                    ZStack(alignment: .leading) {
                        Capsule().fill(Theme.hairline)
                        Capsule().fill(fill).frame(width: proxy.size.width * meter.fraction)
                    }
                }
                .frame(height: 10)
                .accessibilityElement()
                .accessibilityLabel(meter.spoken)
                if let text = meter.stateText {
                    Text(text).font(.footnote).foregroundStyle(meter.state == .ok ? Color.secondary : fill).wrapping()
                }
            }
        }
    }
}

/// The same numbers in figures, for anyone who cannot tell the colors apart, and to copy from.
private struct EffortTable: View {
    let rows: [TeamEffortReport.TableRow]
    @State private var isOpen = false

    var body: some View {
        DisclosureGroup(Ja.Effort.tableShow, isExpanded: $isOpen) {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(rows) { row in
                    VStack(alignment: .leading, spacing: 2) {
                        HStack {
                            Text(row.label).font(.subheadline.weight(.semibold)).wrapping()
                            Text(row.cost).font(.subheadline.monospacedDigit())
                        }
                        if !row.share.isEmpty {
                            Text("\(Ja.Effort.tableShare) \(row.share) · \(Ja.Effort.tableIntended) \(row.intended)").font(.caption).foregroundStyle(.secondary).wrapping()
                        }
                    }
                }
            }
            .padding(.top, 6)
        }
        .font(.subheadline)
    }
}
#endif
