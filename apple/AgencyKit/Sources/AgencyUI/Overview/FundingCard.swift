#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// One funding source: its weekly and five-hour windows, each projected to its reset.
struct FundingCard: View {
    let source: FundingSummary
    let now: Date

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                ExpandableTitle(text: source.label)
                Spacer(minLength: 0)
                Text(Ja.Funding.kind(source.kind)).font(.caption).foregroundStyle(.secondary).fixedSize()
            }
            if source.windows.isEmpty {
                Text(Ja.Funding.unknownUsage).font(.subheadline).foregroundStyle(.secondary).wrapping()
            } else {
                ForEach(source.windows, id: \.name) { WindowBlock(reading: $0, now: now) }
            }
            if let readAt = source.readAt {
                Text(Ja.Funding.readAt(JaFormat.dateTime(readAt.date))).font(.caption).foregroundStyle(.secondary).wrapping()
            }
        }
        .card()
    }
}

private struct WindowBlock: View {
    let reading: WindowReading
    let now: Date

    private var label: String { Ja.Funding.window(reading.name) }

    var body: some View {
        switch reading {
        case .notStarted:
            Text(Ja.Funding.notStarted(label)).font(.subheadline).foregroundStyle(.secondary).wrapping()
        case .stale:
            Text(Ja.Funding.stale(label)).font(.subheadline).foregroundStyle(.secondary).wrapping()
        case .projected(_, let forecast):
            projected(forecast)
        }
    }

    private func projected(_ forecast: WindowForecast) -> some View {
        let used = JaFormat.percent(Double(forecast.utilization))
        let expected = JaFormat.percent(forecast.projectedAtReset)
        return VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(label).font(.subheadline.weight(.semibold)).wrapping()
                Tag(text: Ja.Funding.status(forecast.status), color: color(forecast.status))
            }
            Text("\(used) · \(Ja.Funding.left(JaFormat.percent(max(0, 100 - Double(forecast.utilization)))))")
                .font(.subheadline).wrapping()
            UsageBar(used: min(100, Double(forecast.utilization)), expected: min(100, forecast.projectedAtReset), color: color(forecast.status))
                .accessibilityLabel(Ja.Funding.barLabel(label, used: used, projected: expected))
            Text(line(forecast, expected: expected)).font(.footnote).foregroundStyle(.secondary).wrapping()
        }
    }

    private func line(_ forecast: WindowForecast, expected: String) -> String {
        let reset = Ja.Funding.resetLine(
            JaFormat.dateTime(forecast.resetsAt), relative: Ago.text(from: forecast.resetsAt, now: now), projected: expected
        )
        let hit = forecast.hitAt.map { Ja.Funding.hitLine(JaFormat.dateTime($0)) }
        return [reset, hit, Ja.Funding.verdict(forecast.status)].compactMap { $0 }.joined(separator: " ")
    }

    private func color(_ status: WindowStatus) -> Color {
        switch status {
        case .waste: .secondary
        case .ok: .green
        case .overrun: Theme.danger
        }
    }
}

/// The share used, and the share expected by the reset if the pace holds.
private struct UsageBar: View {
    let used: Double
    let expected: Double
    let color: Color

    var body: some View {
        GeometryReader { proxy in
            ZStack(alignment: .leading) {
                Capsule().fill(Theme.hairline)
                Capsule().fill(color.opacity(0.35)).frame(width: proxy.size.width * expected / 100)
                Capsule().fill(color).frame(width: proxy.size.width * used / 100)
            }
        }
        .frame(height: 8)
    }
}
#endif
