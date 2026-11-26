#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// What the org's work cost on the days a KPI is drawn over, as columns under its trend, so that a movement can be read next to the
/// effort of its days. A day with no cost reported is a mark and not a column (it is not a cost of 0), a tap says a day in words, and
/// nothing is drawn before the cost has been read but a quiet bar. It is a few columns of a small height: the card says the number.
struct CostColumns: View {
    /// `nil` while the cost is being read.
    let cost: OrgCost?
    @State private var selected: Int?

    var body: some View {
        switch cost {
        case nil:
            SkeletonLines(lines: 1)
        case .unavailable?:
            Text(Ja.Outcomes.Cost.unavailable).font(.caption).foregroundStyle(.secondary).wrapping()
        case .ready(let days)?:
            columns(days)
        }
    }

    @ViewBuilder
    private func columns(_ cost: CostOnDays) -> some View {
        if cost.known.isEmpty {
            Text(Ja.Outcomes.Cost.none).font(.caption).foregroundStyle(.secondary).wrapping()
            if let partial = cost.partialLine { Text(partial).font(.caption).foregroundStyle(.secondary).wrapping() }
        } else {
            VStack(alignment: .leading, spacing: 4) {
                Text(Ja.Outcomes.Cost.title).font(.caption).foregroundStyle(.secondary)
                HStack(alignment: .bottom, spacing: cost.days.count > 40 ? 1 : 2) {
                    ForEach(Array(cost.days.enumerated()), id: \.offset) { index, day in
                        column(day, maximum: cost.maximum, isSelected: selected == index)
                            .contentShape(Rectangle())
                            .onTapGesture { selected = selected == index ? nil : index }
                    }
                }
                .frame(height: 44)
                .accessibilityElement(children: .ignore)
                .accessibilityLabel(Ja.Outcomes.Cost.title)
                .accessibilityValue(cost.known.map { cost.describe($0) }.joined(separator: "、"))
                if let selected, cost.days.indices.contains(selected) {
                    Text(cost.describe(cost.days[selected])).font(.caption.weight(.semibold)).wrapping()
                }
                if let partial = cost.partialLine { Text(partial).font(.caption).foregroundStyle(.secondary).wrapping() }
            }
        }
    }

    @ViewBuilder
    private func column(_ day: CostDay, maximum: Double, isSelected: Bool) -> some View {
        VStack(spacing: 0) {
            Spacer(minLength: 0)
            if let usd = day.usd {
                Rectangle()
                    .fill(isSelected ? Color.accentColor : Color.accentColor.opacity(0.55))
                    .frame(height: usd > 0 ? max(1, 44 * min(usd / max(maximum, 0.0001), 1)) : 0)
            } else {
                // No team reported this day: a mark on the baseline, which is not a column of 0.
                Rectangle().fill(Theme.hairline).frame(height: 1)
            }
        }
        .frame(maxWidth: .infinity)
    }
}
#endif
