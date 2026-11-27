#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// The p95 hour by hour, one small chart for each route group (and for the in-node reads against the budget). For a range of more than
/// 7 days the node gives totals only, and the card says so and draws nothing. Nothing is drawn when no hour had a call.
struct HourTrendCard: View {
    let trend: NodeSpeed.Trend
    /// Whether each chart shows its table of hours from the start: for a rendering that shows them.
    var showsTable = false

    var body: some View {
        switch trend {
        case .onlyShort:
            card { Text(Ja.NodeSpeed.Trend.onlyShort).font(.subheadline).foregroundStyle(.secondary).wrapping() }
        case .charts(let charts) where charts.isEmpty:
            EmptyView()
        case .charts(let charts):
            card {
                Text(Ja.NodeSpeed.Trend.lead).font(.footnote).foregroundStyle(.secondary).wrapping()
                ForEach(charts) { chart in HourChartView(chart: chart, showsTable: showsTable) }
            }
        }
    }

    private func card<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            Text(Ja.NodeSpeed.Trend.title).font(.headline).wrapping()
            content()
        }
        .card()
    }
}

/// One group's p95 for each hour, on one axis in milliseconds (a log scale). A gap is an hour with no call; an hour slower than the last
/// bound is a hollow mark at the top. For anyone who cannot see the picture the same hours are a table under it.
struct HourChartView: View {
    let chart: HourChart
    var showsTable = false
    private static let plotHeight: CGFloat = 130
    private static let axisWidth: CGFloat = 62

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(chart.title).font(.subheadline.weight(.semibold)).wrapping()
            // The budget is named in the flow of the text, not on the plot, so that it never runs into the plot at a large text size.
            if let budget = chart.budgetMs {
                Label(Ja.NodeSpeed.Trend.budget(Ja.NodeSpeed.duration(budget)), systemImage: "minus").font(.caption).foregroundStyle(.secondary)
                    .labelStyle(.titleAndIcon)
            }
            HStack(alignment: .top, spacing: 6) {
                axis.frame(width: Self.axisWidth, height: Self.plotHeight)
                VStack(alignment: .leading, spacing: 4) {
                    plot.frame(height: Self.plotHeight)
                    times
                }
            }
            if !chart.isExact { Text(Ja.NodeSpeed.Trend.approximateNote).font(.caption).foregroundStyle(.secondary).wrapping() }
            if !chart.isExact, chart.budgetMs != nil { Text(Ja.NodeSpeed.Trend.budgetNote).font(.caption).foregroundStyle(.secondary).wrapping() }
            HoursTable(chart: chart, isOpen: showsTable)
        }
    }

    private var axis: some View {
        GeometryReader { proxy in
            ForEach(HourScale.ticksMs, id: \.self) { tick in
                Text(Ja.NodeSpeed.duration(tick)).font(.caption2).foregroundStyle(.secondary).lineLimit(1).minimumScaleFactor(0.6)
                    .frame(width: proxy.size.width, alignment: .trailing)
                    .position(x: proxy.size.width / 2, y: y(HourScale.share(ms: Double(tick)), in: proxy.size))
            }
        }
        .accessibilityHidden(true)
    }

    private var plot: some View {
        GeometryReader { proxy in
            ZStack(alignment: .topLeading) {
                ForEach(HourScale.ticksMs, id: \.self) { tick in
                    Rectangle().fill(Theme.hairline).frame(height: 1).position(x: proxy.size.width / 2, y: y(HourScale.share(ms: Double(tick)), in: proxy.size))
                }
                if let budget = chart.budgetMs { budgetLine(budget, in: proxy.size) }
                linePath(in: proxy.size).stroke(Color.accentColor, style: StrokeStyle(lineWidth: 2, lineCap: .round, lineJoin: .round))
                ForEach(Array(chart.points.enumerated()), id: \.element.id) { index, point in
                    if let share = HourScale.share(of: point.bound) { mark(for: point, at: index, share: share, in: proxy.size) }
                }
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(chart.summary ?? chart.title)
    }

    private func budgetLine(_ budget: Int, in size: CGSize) -> some View {
        let height = y(HourScale.share(ms: Double(budget)), in: size)
        return Path { path in
            path.move(to: CGPoint(x: 0, y: height))
            path.addLine(to: CGPoint(x: size.width, y: height))
        }
        .stroke(Color.secondary, style: StrokeStyle(lineWidth: 1, dash: [4, 3]))
    }

    @ViewBuilder
    private func mark(for point: HourPoint, at index: Int, share: Double, in size: CGSize) -> some View {
        let center = CGPoint(x: x(index, in: size), y: y(share, in: size))
        if point.bound == .beyond {
            Circle().strokeBorder(Color.accentColor, lineWidth: 2).background(Circle().fill(Color(white: 1, opacity: 0.001))).frame(width: 8, height: 8).position(center)
        } else {
            Circle().fill(Color.accentColor).frame(width: 5, height: 5).position(center)
        }
    }

    private var times: some View {
        HStack {
            if let first = chart.points.first, let last = chart.points.last {
                Text(JaFormat.dateTime(first.at))
                Spacer(minLength: 0)
                Text(JaFormat.dateTime(last.at))
            }
        }
        .font(.caption2).foregroundStyle(.secondary)
        .accessibilityHidden(true)
    }

    private func x(_ index: Int, in size: CGSize) -> CGFloat { (CGFloat(index) + 0.5) / CGFloat(max(chart.points.count, 1)) * size.width }
    private func y(_ share: Double, in size: CGSize) -> CGFloat { size.height * (1 - CGFloat(share)) }

    /// A line that stops where an hour had no call and starts again after it: a gap is not a 0.
    private func linePath(in size: CGSize) -> Path {
        var path = Path()
        var isBroken = true
        for (index, point) in chart.points.enumerated() {
            guard let share = HourScale.share(of: point.bound) else { isBroken = true; continue }
            let next = CGPoint(x: x(index, in: size), y: y(share, in: size))
            if isBroken { path.move(to: next) } else { path.addLine(to: next) }
            isBroken = false
        }
        return path
    }
}

/// The hours with calls, newest first, as a table: the same numbers as the picture, for anyone who cannot see it.
private struct HoursTable: View {
    let chart: HourChart
    @State private var isOpen: Bool

    init(chart: HourChart, isOpen: Bool) {
        self.chart = chart
        _isOpen = State(initialValue: isOpen)
    }

    var body: some View {
        DisclosureGroup(Ja.NodeSpeed.Trend.show, isExpanded: $isOpen) {
            VStack(alignment: .leading, spacing: 4) {
                row(Ja.NodeSpeed.Trend.hour, Ja.NodeSpeed.Trend.calls, "p95", isHeader: true)
                ForEach(chart.rows) { line in row(line.hour, line.calls, line.p95, isHeader: false) }
            }
            .accessibilityElement(children: .contain)
            .accessibilityLabel(Ja.NodeSpeed.Trend.caption(chart.title))
        }
        .font(.subheadline)
    }

    private func row(_ hour: String, _ calls: String, _ p95: String, isHeader: Bool) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(hour).frame(maxWidth: .infinity, alignment: .leading)
            Text(calls).monospacedDigit().frame(width: 56, alignment: .trailing)
            Text(p95).monospacedDigit().frame(width: 104, alignment: .trailing)
        }
        .font(isHeader ? .caption.weight(.semibold) : .footnote)
        .foregroundStyle(isHeader ? .secondary : .primary)
        .accessibilityElement(children: .combine)
    }
}
#endif
