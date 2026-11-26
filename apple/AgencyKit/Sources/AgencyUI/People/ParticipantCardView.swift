#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// One participant: their face, what they are doing, and what they hold.
struct ParticipantCardView: View {
    let card: ParticipantCard
    /// Opens the desk. `nil` draws the line without a button, for renderings.
    var onOpenDesk: (() -> Void)?
    @State private var isExpanded = false

    var body: some View {
        HStack(alignment: .top, spacing: Theme.spacing) {
            Face(label: card.person.label, kind: card.person.kind, presence: card.presence)
            VStack(alignment: .leading, spacing: 4) {
                header
                if !card.person.status.isEmpty { Text(card.person.status).font(.subheadline).wrapping() }
                if card.presence == .needsYou { waitingLine }
                if let since = card.since {
                    Text(Ja.People.since(JaFormat.dateTime(since))).font(.caption).foregroundStyle(.secondary).wrapping()
                }
                if !card.person.holding.isEmpty { holding }
                if card.presence == .needsYou, let onOpenDesk {
                    Button(Ja.People.openDecisions, action: onOpenDesk).font(.subheadline.weight(.semibold))
                }
            }
        }
        .card()
        .accessibilityElement(children: .contain)
    }

    private var header: some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            ExpandableTitle(text: card.person.label)
            if card.isLead { Tag(text: Ja.People.roleLead, color: .secondary) }
            if let tag = card.stateTag { Tag(text: tag, color: color) }
        }
    }

    private var waitingLine: some View {
        Text(card.waitingForTitle.map { "\(Ja.People.waitingForAnswer): 「\($0)」" } ?? Ja.People.waitingForAnswer)
            .font(.footnote).foregroundStyle(Color.accentColor).wrapping()
    }

    private var holding: some View {
        let shown = PeopleLimits.shown(card.person.holding, limit: PeopleLimits.heldShown, isExpanded: isExpanded)
        return VStack(alignment: .leading, spacing: 2) {
            Text(Ja.People.holding).font(.caption).foregroundStyle(.secondary)
            Text(shown.items.joined(separator: "  ")).font(.caption.monospaced()).wrapping()
            if shown.hidden > 0 {
                Button(Ja.People.moreHeld(shown.hidden)) { isExpanded = true }.font(.caption)
            }
        }
        .padding(.top, 2)
    }

    private var color: Color {
        switch card.presence {
        case .needsYou: .accentColor
        case .working: .green
        case .waiting: Theme.warning
        case .away, .idle: .secondary
        }
    }
}
#endif
