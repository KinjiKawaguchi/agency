#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// The briefing and the pile of requests, grouped. The content only: the screen around it adds
/// navigation, pull to reload and the banners.
struct DeskPile: View {
    let items: [DeskItem]
    let situation: Situation
    let now: Date
    /// Draws the rows without links, for renderings made outside a navigation stack.
    var isStatic = false

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            briefing
            if items.isEmpty {
                Text(Ja.Desk.empty)
                    .font(.body)
                    .foregroundStyle(.secondary)
                    .wrapping()
                    .card()
            }
            ForEach(DeskGroup.allCases, id: \.self) { group in
                let inGroup = items.filter { $0.group == group }
                if !inGroup.isEmpty {
                    Text(Ja.Desk.group(group))
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(.secondary)
                        .padding(.top, 8)
                        .wrapping()
                    ForEach(inGroup) { item in
                        let row = RequestRow(item: item, now: now)
                        if isStatic {
                            row
                        } else {
                            NavigationLink(value: item.id) { row }.buttonStyle(.plain)
                        }
                    }
                }
            }
        }
    }

    /// One line on what is waiting, and on how the team is doing.
    private var briefing: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(situation.mine == 0 ? Ja.Desk.nothingToDecide : Ja.Desk.itemsOnDesk(situation.mine))
                .font(.title3.weight(.semibold))
                .wrapping()
            if situation.waitingOnYou > 0 {
                Text(Ja.Desk.someoneWaiting(situation.waitingOnYou))
                    .font(.subheadline)
                    .wrapping()
            }
            if situation.total > 0 {
                Text(Ja.Desk.peopleWorking(working: situation.working, total: situation.total))
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .wrapping()
            }
        }
    }
}
#endif
