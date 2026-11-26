#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// One team's token use under its heading. A team whose runtime does not report usage says so, in place of numbers.
struct TeamTokens: View {
    let team: CostsTeam
    var selectedDay: String?
    var onTryAgain: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            Text(Ja.Costs.tokensTitle(team.team.view.label)).font(.title3.weight(.semibold)).wrapping()
            switch team.usage {
            case .loading:
                SkeletonCard()
            case .unavailable:
                Text(Ja.Costs.unavailable).font(.subheadline).foregroundStyle(.secondary).wrapping()
            case .couldNotAnswer(let reason), .failed(let reason):
                FailedRegion(what: Ja.Costs.tokensTitle(team.team.view.label), why: reason, onTryAgain: onTryAgain)
            case .ready:
                if let report = team.report { ReportView(report: report, selectedDay: selectedDay) }
            }
        }
    }
}

private struct ReportView: View {
    let report: TokenReport
    let selectedDay: String?

    var body: some View {
        ForEach(report.tiles) { tile in
            VStack(alignment: .leading, spacing: 2) {
                Text(tile.label).font(.caption).foregroundStyle(.secondary).wrapping()
                Text(tile.value).font(.title.weight(.bold).monospacedDigit())
                Text(tile.note).font(.caption).foregroundStyle(.secondary).wrapping()
            }
            .card()
            .accessibilityElement(children: .combine)
        }
        VStack(alignment: .leading, spacing: 8) {
            Text(Ja.Costs.byDay).font(.subheadline.weight(.semibold)).wrapping()
            Text(Ja.Costs.byDayHint).font(.caption).foregroundStyle(.secondary).wrapping()
            SeriesLegend(series: report.series)
            StackedColumns(columns: report.columns, series: report.series, selected: selectedDay)
        }
        .card()
        Text(Ja.Costs.byParticipant).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary).padding(.top, 4).wrapping()
        ForEach(report.rows) { ParticipantUsage(row: $0) }
    }
}

private struct ParticipantUsage: View {
    let row: TokenReport.Row

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                ExpandableTitle(text: row.label, font: .subheadline.weight(.semibold))
                Text(row.cost).font(.subheadline.weight(.semibold).monospacedDigit()).fixedSize()
            }
            figure(Ja.Costs.outputColumn, row.output)
            figure(Ja.Costs.cacheReadColumn, row.cacheRead)
            figure(Ja.Costs.inputWriteColumn, row.inputWrite)
        }
        .card()
        .accessibilityElement(children: .combine)
    }

    private func figure(_ name: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(name).font(.caption).foregroundStyle(.secondary).wrapping()
            Text(value).font(.caption.monospacedDigit())
        }
    }
}
#endif
