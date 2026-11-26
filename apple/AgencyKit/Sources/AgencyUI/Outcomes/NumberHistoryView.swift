#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// The history of a KPI's numbers under its card: a button to open it, and, open, every entry newest first with where it came
/// from and when it was entered. An entry a later one for its day replaced is kept and marked, not hidden.
struct NumberHistoryView: View {
    let state: HistoryState?
    let onToggle: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Button(state == nil ? Ja.History.show : Ja.History.hide, action: onToggle)
                .buttonStyle(.bordered)
                .font(.subheadline)
            switch state {
            case nil:
                EmptyView()
            case .loading?:
                SkeletonCard(lines: 2)
            case .failed(let reason)?:
                Text(Ja.Desk.loadFailed(reason)).font(.footnote).foregroundStyle(Theme.danger).wrapping()
            case .entries(let entries)?:
                if entries.isEmpty {
                    Text(Ja.History.empty).font(.footnote).foregroundStyle(.secondary).wrapping()
                }
                ForEach(entries) { EntryRow(entry: $0) }
            }
        }
    }
}

private struct EntryRow: View {
    let entry: NumberHistoryEntry

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(entry.day).font(.subheadline.weight(.semibold))
                Text(entry.value).font(.subheadline)
            }
            Text("\(entry.source)・\(entry.enteredOn)").font(.caption).foregroundStyle(.secondary).wrapping()
            if let note = entry.note { Text(note).font(.caption).wrapping() }
            if entry.isReplaced { Text(Ja.History.replaced).font(.caption.italic()).foregroundStyle(.secondary).wrapping() }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .opacity(entry.isReplaced ? 0.7 : 1)
        .accessibilityElement(children: .combine)
    }
}

extension Ja {
    fileprivate typealias History = Ja.Outcomes.History
}
#endif
