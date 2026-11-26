#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// One chart of the evaluation in a card: its title, who is who, the plot, and the day that was tapped.
struct EvaluationChartView: View {
    let chart: EvaluationChart
    @State private var selected: Int?

    init(chart: EvaluationChart, selectedDay: String? = nil) {
        self.chart = chart
        _selected = State(initialValue: chart.days.firstIndex { $0.date == selectedDay })
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            VStack(alignment: .leading, spacing: 2) {
                Text(chart.title).font(.subheadline.weight(.semibold)).wrapping()
                if let hint = chart.hint { Text(hint).font(.caption).foregroundStyle(.secondary).wrapping() }
            }
            legend
            if chart.hasNoData {
                Text(Ja.Evaluation.noSamples).font(.footnote).foregroundStyle(.secondary).wrapping()
            }
            ChartFrame(axis: chart.axis, dateLabels: chart.dateLabels) { plot }
            if let selected, chart.days.indices.contains(selected) { detail(chart.days[selected]) }
        }
        .card()
    }

    @ViewBuilder
    private var plot: some View {
        let colors = chart.series.map { SeriesColor.slot($0.slot) }
        switch chart.kind {
        case .columns:
            ColumnPlot(values: chart.days.map { $0.values.map { $0 ?? 0 } }, colors: colors, maximum: chart.max, selected: $selected)
        case .lines:
            LinePlot(lines: chart.series.indices.map { line in chart.days.map { $0.values[line] } }, colors: colors, maximum: chart.max, selected: $selected)
        }
    }

    /// Who is who, when there is more than one series.
    @ViewBuilder
    private var legend: some View {
        if chart.series.count >= 2 {
            FlowLine(spacing: 12) {
                ForEach(chart.series) { one in
                    HStack(spacing: 4) {
                        Circle().fill(SeriesColor.slot(one.slot)).frame(width: 8, height: 8)
                        Text(one.label).font(.caption).lineLimit(1)
                    }
                }
            }
        }
    }

    private func detail(_ day: ChartDay) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(day.shortDate + (day.totalText.map { "  " + $0 } ?? "")).font(.footnote.weight(.semibold))
            if chart.series.count > 1 {
                ForEach(Array(chart.series.enumerated()), id: \.offset) { index, one in
                    if chart.kind == .lines || (day.values[index] ?? 0) > 0 {
                        HStack(spacing: 6) {
                            Circle().fill(SeriesColor.slot(one.slot)).frame(width: 8, height: 8)
                            Text(one.label).font(.caption).lineLimit(3).wrapping()
                            Text(day.valueTexts[index]).font(.caption.monospacedDigit())
                        }
                    }
                }
            } else if let text = day.valueTexts.first {
                Text(text).font(.caption.monospacedDigit())
            }
        }
        .card()
        .accessibilityLabel(day.spoken)
    }
}
#endif
