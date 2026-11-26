#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// One team's evaluation: the headline numbers, then each of them over the days.
struct TeamEvaluationView: View {
    let report: TeamEvaluationReport
    var selectedDay: String?

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            Text(report.title).font(.title3.weight(.semibold)).wrapping()
            tiles
            ForEach(report.charts) { EvaluationChartView(chart: $0, selectedDay: selectedDay) }
            // The owner's extra: after the charts, and only when the node gave participants.
            if let participants = report.participants { ParticipantsView(section: participants).padding(.top, 8) }
        }
    }

    /// Two to a row, the numbers a person watches first on top.
    private var tiles: some View {
        LazyVGrid(columns: [GridItem(.flexible(), spacing: Theme.spacing, alignment: .top), GridItem(.flexible(), spacing: Theme.spacing, alignment: .top)], spacing: Theme.spacing) {
            ForEach(report.tiles) { tile in
                VStack(alignment: .leading, spacing: 2) {
                    Text(tile.label).font(.caption).foregroundStyle(.secondary).wrapping()
                    Text(tile.value).font(.title2.weight(.bold).monospacedDigit()).lineLimit(1).minimumScaleFactor(0.6)
                    Text(tile.note).font(.caption2).foregroundStyle(.secondary).wrapping()
                }
                .frame(maxHeight: .infinity, alignment: .topLeading)
                .card()
                .accessibilityElement(children: .combine)
            }
        }
    }
}
#endif
