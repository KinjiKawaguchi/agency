#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// Every org and team on one screen, the ones that need the viewer first, then the funding sources.
/// The content only: the screen around it adds the title, pull to reload and the banners.
struct OverviewContent: View {
    let sections: [OverviewStore.OrgSection]
    let waiting: (requests: Int, teams: Int)
    let funding: [FundingSummary]
    let cost: (TeamEntry) -> WeekCost
    let now: Date
    var onOpenDesk: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            Text(Ja.Overview.lead(requests: waiting.requests, teams: waiting.teams)).font(.subheadline).foregroundStyle(.secondary).wrapping()
            ForEach(sections, id: \.org.id) { section in
                orgSection(section)
            }
            if !funding.isEmpty {
                Text(Ja.Overview.funding).font(.title3.weight(.semibold)).padding(.top, 12).wrapping()
                ForEach(funding, id: \.id) { FundingCard(source: $0, now: now) }
            }
        }
    }

    @ViewBuilder
    private func orgSection(_ section: OverviewStore.OrgSection) -> some View {
        Text(section.org.label).font(.title3.weight(.semibold)).padding(.top, 8).wrapping()
        if section.teams.isEmpty {
            Text(Ja.Overview.noTeams).font(.subheadline).foregroundStyle(.secondary).wrapping()
        }
        ForEach(section.teams, id: \.key) { entry in
            TeamCard(entry: entry, cost: cost(entry), now: now, onOpenDesk: onOpenDesk)
        }
    }
}
#endif
