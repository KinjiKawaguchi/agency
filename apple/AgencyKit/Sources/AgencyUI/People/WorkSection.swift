#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// Work in progress, and below it the work that cannot move.
struct WorkSection: View {
    let doing: [WorkRow]
    let blocked: [WorkRow]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Heading(text: Ja.People.workTitle)
            if doing.isEmpty {
                Text(Ja.People.noWork).font(.subheadline).foregroundStyle(.secondary).wrapping()
            } else {
                WorkRows(rows: doing)
            }
            if !blocked.isEmpty {
                Heading(text: Ja.People.blockedTitle)
                WorkRows(rows: blocked)
            }
        }
    }
}

struct Heading: View {
    let text: String

    var body: some View {
        Text(text).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary).padding(.top, 8).wrapping()
    }
}

private struct WorkRows: View {
    let rows: [WorkRow]
    @State private var isExpanded = false

    var body: some View {
        let shown = PeopleLimits.shown(rows, limit: PeopleLimits.rowsShown, isExpanded: isExpanded)
        VStack(alignment: .leading, spacing: 8) {
            ForEach(shown.items) { WorkRowView(row: $0) }
            if shown.hidden > 0 {
                Button(Ja.People.moreWork(shown.hidden)) { isExpanded = true }.font(.subheadline).buttonStyle(.bordered)
            }
        }
    }
}

private struct WorkRowView: View {
    let row: WorkRow

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Face(label: row.assigneeLabel, kind: row.assigneeKind, size: .small)
            VStack(alignment: .leading, spacing: 3) {
                ExpandableTitle(text: row.title, font: .subheadline.weight(.semibold), lineLimit: 3)
                Text(row.ref + " · " + row.assigneeLabel).font(.caption.monospaced()).foregroundStyle(.secondary).wrapping()
                if let text = row.blockedText { Text(text).font(.caption).foregroundStyle(Theme.warning).wrapping() }
                if let eta = row.etaText { Text(eta).font(.caption).foregroundStyle(.secondary).wrapping() }
            }
        }
        .card()
        .accessibilityElement(children: .combine)
    }
}
#endif
