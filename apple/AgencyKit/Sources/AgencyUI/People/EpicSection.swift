#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// Epic forecasts: how far along each epic is and when it should finish.
struct EpicSection: View {
    let epics: [EpicRow]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Heading(text: Ja.People.epicsTitle)
            if epics.isEmpty {
                Text(Ja.People.noEpics).font(.subheadline).foregroundStyle(.secondary).wrapping()
            } else {
                legend
                ForEach(epics) { EpicRowView(epic: $0) }
            }
        }
    }

    private var legend: some View {
        HStack(spacing: 12) {
            ForEach([Work.SegmentKind.done, .doing, .blocked, .rest], id: \.self) { kind in
                HStack(spacing: 4) {
                    Circle().fill(EpicBar.color(kind)).frame(width: 8, height: 8)
                    Text(Self.name(kind)).font(.caption).foregroundStyle(.secondary)
                }
            }
        }
        .accessibilityHidden(true)
    }

    private static func name(_ kind: Work.SegmentKind) -> String {
        switch kind {
        case .done: Ja.People.legendDone
        case .doing: Ja.People.legendDoing
        case .blocked: Ja.People.legendBlocked
        case .rest: Ja.People.legendRest
        }
    }
}

private struct EpicRowView: View {
    let epic: EpicRow

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                ExpandableTitle(text: epic.title, font: .subheadline.weight(.semibold), lineLimit: 3)
                Text(epic.count).font(.subheadline.monospacedDigit()).foregroundStyle(.secondary).fixedSize()
            }
            EpicBar(segments: epic.segments).accessibilityLabel(epic.barLabel)
            Text(epic.ref + " · " + epic.meta).font(.caption).foregroundStyle(.secondary).wrapping()
            if let blockedBy = epic.blockedBy { Text(blockedBy).font(.caption).foregroundStyle(Theme.warning).wrapping() }
        }
        .card()
    }
}

/// The bar of an epic: each segment as wide as its share of the epic's work items.
private struct EpicBar: View {
    let segments: [Work.Segment]

    static func color(_ kind: Work.SegmentKind) -> Color {
        switch kind {
        case .done: .green
        case .doing: .accentColor
        case .blocked: Theme.warning
        case .rest: Theme.hairline
        }
    }

    var body: some View {
        GeometryReader { proxy in
            let total = CGFloat(segments.reduce(0) { $0 + $1.count })
            HStack(spacing: 2) {
                ForEach(Array(segments.enumerated()), id: \.offset) { _, segment in
                    Rectangle().fill(Self.color(segment.kind))
                        .frame(width: max(0, (proxy.size.width - 2 * CGFloat(segments.count - 1)) * CGFloat(segment.count) / max(total, 1)))
                }
            }
        }
        .frame(height: 8)
        .clipShape(Capsule())
        .background(Capsule().fill(segments.isEmpty ? Theme.hairline : .clear))
    }
}
#endif
