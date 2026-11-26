#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// The funding sources, then each team's token use. The content only: the screen around it adds the title,
/// pull to reload and the banners.
struct CostsContent: View {
    let funding: [FundingSummary]
    let teams: [CostsTeam]
    let days: Int
    let now: Date
    var onChoose: ((Int) -> Void)?
    var selectedDay: String?
    var onTryAgain: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            Text(Ja.Costs.funding).font(.title3.weight(.semibold)).wrapping()
            if funding.isEmpty {
                Text(Ja.Costs.noFunding).font(.subheadline).foregroundStyle(.secondary).wrapping()
            }
            ForEach(funding, id: \.id) { FundingCard(source: $0, now: now) }
            if !teams.isEmpty { period }
            ForEach(teams) { TeamTokens(team: $0, selectedDay: selectedDay, onTryAgain: onTryAgain).padding(.top, 12) }
        }
    }

    private var period: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(Ja.Costs.period).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary).padding(.top, 12)
            HStack(spacing: 8) {
                ForEach(CostsStore.periods, id: \.self) { period in
                    Button(Ja.Costs.days(period)) { onChoose?(period) }
                        .buttonStyle(PeriodStyle(isSelected: period == days))
                }
            }
            Text(Ja.Costs.estimate).font(.caption).foregroundStyle(.secondary).wrapping()
        }
    }
}

/// A period button: filled when it is the chosen one. The costs and the outcomes offer their periods the same way.
struct PeriodStyle: ButtonStyle {
    let isSelected: Bool

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.subheadline.weight(.semibold))
            .padding(.horizontal, 14)
            .padding(.vertical, 6)
            .foregroundStyle(isSelected ? Color.white : Color.accentColor)
            .background(isSelected ? Color.accentColor : Color.accentColor.opacity(0.12), in: Capsule())
            .opacity(configuration.isPressed ? 0.7 : 1)
            .accessibilityAddTraits(isSelected ? .isSelected : [])
    }
}
#endif
