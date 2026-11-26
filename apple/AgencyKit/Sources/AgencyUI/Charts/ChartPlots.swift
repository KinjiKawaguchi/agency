#if canImport(SwiftUI)
import SwiftUI

/// The axes and hairline grid every chart of the app shares: ticks on the left, days underneath, the plot between.
struct ChartFrame<Plot: View>: View {
    /// The ticks, top to bottom.
    let axis: [String]
    /// The first, middle and last day, written under the plot.
    let dateLabels: [String]
    var height: CGFloat = 120
    @ViewBuilder var plot: Plot

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(alignment: .top, spacing: 6) {
                VStack(alignment: .trailing, spacing: 0) {
                    ForEach(Array(axis.enumerated()), id: \.offset) { index, tick in
                        if index > 0 { Spacer(minLength: 0) }
                        Text(tick).font(.caption2).foregroundStyle(.secondary).lineLimit(1).minimumScaleFactor(0.7)
                    }
                }
                .frame(width: 46, height: height)
                plot
                    .frame(height: height)
                    .background(alignment: .top) { grid }
            }
            if !dateLabels.isEmpty {
                HStack(spacing: 6) {
                    Color.clear.frame(width: 46, height: 0)
                    HStack {
                        Text(dateLabels[0])
                        Spacer(minLength: 0)
                        if dateLabels.count > 2 { Text(dateLabels[1]) }
                        Spacer(minLength: 0)
                        Text(dateLabels[dateLabels.count - 1])
                    }
                }
                .font(.caption2).foregroundStyle(.secondary)
            }
        }
    }

    private var grid: some View {
        VStack(spacing: 0) {
            Rectangle().fill(Theme.hairline).frame(height: 1)
            Spacer(minLength: 0)
            Rectangle().fill(Theme.hairline).frame(height: 1)
            Spacer(minLength: 0)
            Rectangle().fill(Theme.hairline).frame(height: 1)
        }
    }
}

/// Columns by day, each a stack of its values (the first at the bottom), as tall as its share of `maximum`.
/// A tap on a day selects it.
struct ColumnPlot: View {
    /// For each day, one value for each series.
    let values: [[Double]]
    let colors: [Color]
    let maximum: Double
    @Binding var selected: Int?
    var height: CGFloat = 120

    var body: some View {
        HStack(alignment: .bottom, spacing: values.count > 20 ? 1 : 3) {
            ForEach(Array(values.enumerated()), id: \.offset) { index, day in
                stack(day)
                    .frame(maxWidth: .infinity)
                    .frame(height: height, alignment: .bottom)
                    .background(Rectangle().fill(selected == index ? Theme.hairline : .clear))
                    .contentShape(Rectangle())
                    .onTapGesture { selected = selected == index ? nil : index }
            }
        }
    }

    private func stack(_ day: [Double]) -> some View {
        let total = day.reduce(0, +)
        return VStack(spacing: 0) {
            ForEach(Array(day.enumerated()), id: \.offset) { index, value in
                if value > 0 {
                    Rectangle().fill(colors[index]).frame(height: max(1, height * min(total / max(maximum, 0.0001), 1) * value / max(total, 0.0001)))
                }
            }
        }
    }
}

/// Lines over days, a day to a slot, the same slots as the columns of the other charts. A missing value breaks the
/// line. A tap or a drag on a day selects it.
struct LinePlot: View {
    /// For each line, one value for each day; `nil` where there is nothing to say.
    let lines: [[Double?]]
    let colors: [Color]
    let maximum: Double
    @Binding var selected: Int?
    var height: CGFloat = 120

    private var dayCount: Int { lines.first?.count ?? 0 }

    var body: some View {
        GeometryReader { proxy in
            ZStack(alignment: .topLeading) {
                if let selected { marker(at: selected, in: proxy.size) }
                ForEach(Array(lines.enumerated()), id: \.offset) { index, values in
                    path(values, in: proxy.size).stroke(colors[index], style: StrokeStyle(lineWidth: 2, lineCap: .round, lineJoin: .round))
                    if let last = values.lastIndex(where: { $0 != nil }), let value = values[last] {
                        Circle().fill(colors[index]).frame(width: 7, height: 7)
                            .position(point(last, value, in: proxy.size))
                    }
                }
            }
            .contentShape(Rectangle())
            .gesture(DragGesture(minimumDistance: 0).onChanged { drag in
                guard dayCount > 0 else { return }
                selected = min(max(Int(drag.location.x / proxy.size.width * CGFloat(dayCount)), 0), dayCount - 1)
            })
        }
    }

    private func x(_ index: Int, in size: CGSize) -> CGFloat { (CGFloat(index) + 0.5) / CGFloat(max(dayCount, 1)) * size.width }

    private func point(_ index: Int, _ value: Double, in size: CGSize) -> CGPoint {
        CGPoint(x: x(index, in: size), y: size.height * (1 - CGFloat(min(value / max(maximum, 0.0001), 1))))
    }

    private func path(_ values: [Double?], in size: CGSize) -> Path {
        var path = Path()
        var previousWasMissing = true
        for (index, value) in values.enumerated() {
            guard let value else { previousWasMissing = true; continue }
            let next = point(index, value, in: size)
            if previousWasMissing { path.move(to: next) } else { path.addLine(to: next) }
            previousWasMissing = false
        }
        return path
    }

    private func marker(at index: Int, in size: CGSize) -> some View {
        Rectangle().fill(Theme.hairline).frame(width: 1, height: size.height).position(x: x(index, in: size), y: size.height / 2)
    }
}
#endif
