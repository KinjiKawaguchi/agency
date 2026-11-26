#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// The owner's numbers, org by org: what each KPI stands at, then what the aims came to. The content only: the screen
/// around it adds the banners, pull to reload and the cover over it.
struct OutcomesContent: View {
    let orgs: [OrgOutcomes]
    let days: Int
    var onChoose: ((Int) -> Void)?
    var onTryAgain: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            Text(Ja.Outcomes.note).font(.footnote).foregroundStyle(.secondary).wrapping()
            period
            ForEach(orgs) { org in
                OrgSection(org: org, showsName: orgs.count > 1, onTryAgain: onTryAgain)
            }
        }
    }

    private var period: some View {
        HStack(spacing: 8) {
            ForEach(OutcomesStore.periods, id: \.self) { period in
                Button(Ja.Outcomes.period(period)) { onChoose?(period) }
                    .buttonStyle(PeriodStyle(isSelected: period == days))
            }
        }
    }
}

private struct OrgSection: View {
    let org: OrgOutcomes
    let showsName: Bool
    var onTryAgain: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            if showsName { Text(org.label).font(.title3.weight(.semibold)).wrapping().padding(.top, 8) }
            if org.cards.isEmpty { Text(Ja.Outcomes.noKpis).font(.subheadline).foregroundStyle(.secondary).wrapping() }
            ForEach(org.cards) { KpiCardView(card: $0, from: org.from, until: org.until) }
            ReadingsSection(readings: org.readings, onTryAgain: onTryAgain).padding(.top, 8)
        }
    }
}
#endif
