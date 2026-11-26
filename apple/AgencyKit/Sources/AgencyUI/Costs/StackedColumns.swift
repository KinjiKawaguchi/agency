#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// The colors of the series, in their fixed order: one for each participant, then one for the folded rest.
enum SeriesColor {
    private static let slots: [Color] = [
        Color(red: 0.165, green: 0.471, blue: 0.839), Color(red: 0.922, green: 0.408, blue: 0.204),
        Color(red: 0.106, green: 0.686, blue: 0.478), Color(red: 0.929, green: 0.631, blue: 0.0),
        Color(red: 0.910, green: 0.482, blue: 0.643), Color(red: 0.0, green: 0.514, blue: 0.0),
        Color(red: 0.384, green: 0.314, blue: 0.839), Color(red: 0.890, green: 0.286, blue: 0.282),
    ]
    private static let other = Color(red: 0.561, green: 0.584, blue: 0.631)

    static func of(_ series: UsageChart.Series) -> Color {
        series.slot.map { slots[$0 - 1] } ?? other
    }
}

/// Which participant has which color. Shown only when there is more than one.
struct SeriesLegend: View {
    let series: [UsageChart.Series]

    var body: some View {
        if series.count >= 2 {
            FlowLine(spacing: 12) {
                ForEach(series) { one in
                    HStack(spacing: 4) {
                        Circle().fill(SeriesColor.of(one)).frame(width: 8, height: 8)
                        Text(one.label).font(.caption).lineLimit(1)
                    }
                }
            }
        }
    }
}

/// Cost by day, stacked by participant. A tap on a day says what each participant cost on it, and the
/// cards below carry the same numbers for anyone who cannot tell the colors apart.
struct StackedColumns: View {
    let columns: [UsageChart.Column]
    let series: [UsageChart.Series]
    @State private var selected: String?

    init(columns: [UsageChart.Column], series: [UsageChart.Series], selected: String? = nil) {
        self.columns = columns
        self.series = series
        _selected = State(initialValue: selected)
    }

    private static let height: CGFloat = 120
    private var maximum: Double { max(columns.map(\.total).max() ?? 0, 0.01) }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(JaFormat.money(maximum)).font(.caption2).foregroundStyle(.secondary)
            HStack(alignment: .bottom, spacing: columns.count > 20 ? 1 : 3) {
                ForEach(columns) { column in
                    stack(column)
                        .frame(maxWidth: .infinity)
                        .frame(height: Self.height, alignment: .bottom)
                        .background(Rectangle().fill(selected == column.date ? Theme.hairline : .clear))
                        .contentShape(Rectangle())
                        .onTapGesture { selected = selected == column.date ? nil : column.date }
                        .accessibilityElement()
                        .accessibilityLabel(Ja.Costs.dayTotal(JaFormat.shortDate(column.date), JaFormat.money(column.total)))
                }
            }
            .overlay(alignment: .bottom) { Rectangle().fill(Theme.hairline).frame(height: 1) }
            if let first = columns.first, let last = columns.last {
                HStack {
                    Text(JaFormat.shortDate(first.date))
                    Spacer(minLength: 0)
                    if columns.count > 1 { Text(JaFormat.shortDate(last.date)) }
                }
                .font(.caption2).foregroundStyle(.secondary)
            }
            if let column = columns.first(where: { $0.date == selected }) { detail(column) }
        }
    }

    private func stack(_ column: UsageChart.Column) -> some View {
        VStack(spacing: 0) {
            ForEach(Array(series.enumerated()), id: \.offset) { index, one in
                let value = column.values[index]
                if value > 0 {
                    Rectangle().fill(SeriesColor.of(one))
                        .frame(height: max(1, Self.height * column.total / maximum * value / max(column.total, 0.0001)))
                }
            }
        }
    }

    private func detail(_ column: UsageChart.Column) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(JaFormat.shortDate(column.date) + "  " + JaFormat.money(column.total)).font(.footnote.weight(.semibold))
            ForEach(Array(series.enumerated()), id: \.offset) { index, one in
                HStack(spacing: 6) {
                    Circle().fill(SeriesColor.of(one)).frame(width: 8, height: 8)
                    Text(one.label).font(.caption).lineLimit(2).wrapping()
                    Text(JaFormat.money(column.values[index])).font(.caption.monospacedDigit())
                }
            }
        }
        .card()
    }
}

/// Items in a row that wraps onto the next line instead of running past the screen.
struct FlowLine: Layout {
    var spacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        arrange(subviews, width: proposal.width ?? .infinity).size
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let placement = arrange(subviews, width: bounds.width)
        for (subview, origin) in zip(subviews, placement.origins) {
            // Each item gets the width it was measured at, so a long label is cut instead of running past the row.
            let size = subview.sizeThatFits(ProposedViewSize(width: bounds.width, height: nil))
            subview.place(
                at: CGPoint(x: bounds.minX + origin.x, y: bounds.minY + origin.y),
                proposal: ProposedViewSize(width: min(size.width, bounds.width), height: size.height)
            )
        }
    }

    private func arrange(_ subviews: Subviews, width: CGFloat) -> (size: CGSize, origins: [CGPoint]) {
        var origins: [CGPoint] = []
        var x: CGFloat = 0, y: CGFloat = 0, rowHeight: CGFloat = 0, widest: CGFloat = 0
        for subview in subviews {
            let size = subview.sizeThatFits(ProposedViewSize(width: width, height: nil))
            if x > 0, x + size.width > width { x = 0; y += rowHeight + spacing; rowHeight = 0 }
            origins.append(CGPoint(x: x, y: y))
            x += size.width + spacing
            rowHeight = max(rowHeight, size.height)
            widest = max(widest, x - spacing)
        }
        return (CGSize(width: min(widest, width), height: y + rowHeight), origins)
    }
}
#endif
