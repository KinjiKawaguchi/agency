#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// One team: its participants, its work, its epics.
struct TeamSectionView: View {
    let section: PeopleSection
    let now: Date
    var onOpenDesk: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            Text(section.title).font(.title3.weight(.semibold)).wrapping()
            Text(section.summary).font(.subheadline).foregroundStyle(.secondary).wrapping()
            if let notice = ReplicaNotice.text(for: section.team, now: now) {
                Text(notice).font(.footnote).foregroundStyle(.secondary).wrapping()
            }
            ForEach(section.cards) { ParticipantCardView(card: $0, onOpenDesk: onOpenDesk) }
            WorkSection(doing: section.doing, blocked: section.blocked)
            EpicSection(epics: section.epics)
        }
    }
}

/// Every team's participants, work and epics. The content only: the screen around it adds the title,
/// pull to reload and the banners.
struct PeopleContent: View {
    let sections: [PeopleSection]
    let failures: [String: String]
    let now: Date
    var onOpenDesk: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 28) {
            if sections.isEmpty && failures.isEmpty {
                Text(Ja.People.noTeams).font(.subheadline).foregroundStyle(.secondary).wrapping()
            }
            ForEach(failures.sorted { $0.key < $1.key }, id: \.key) { team, reason in
                Text(Ja.Desk.unreadableTeam(team, reason: reason)).font(.footnote).foregroundStyle(Theme.warning).wrapping()
            }
            ForEach(sections) { TeamSectionView(section: $0, now: now, onOpenDesk: onOpenDesk) }
        }
    }
}
#endif
