#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// One request in the pile.
struct RequestRow: View {
    let item: DeskItem
    let now: Date
    let isSent: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Tag(text: Ja.Desk.kind(item.request.kind), color: item.isMine ? .accentColor : .secondary)
                if item.blockedPeople.count > 0 {
                    Tag(text: Ja.Desk.blocking(item.blockedPeople.count), color: Theme.warning)
                }
                Spacer(minLength: 0)
                if isSent {
                    Image(systemName: "checkmark.circle.fill").foregroundStyle(.green)
                }
            }
            Text(item.request.title)
                .font(.headline)
                .multilineTextAlignment(.leading)
                .lineLimit(4)
                .wrapping()
            Text(Self.meta(of: item, now: now))
                .font(.footnote)
                .foregroundStyle(.secondary)
                .lineLimit(2)
                .wrapping()
            // Whom it is answered for has a line of its own, whole: the label is the owner's own text and may be long.
            if let line = Self.answerLine(of: item) {
                Text(line).font(.footnote.weight(.semibold)).wrapping()
            }
        }
        .card()
        .accessibilityElement(children: .combine)
    }

    /// Who drafted it, the team, and when.
    static func meta(of item: DeskItem, now: Date) -> String {
        let drafter = item.drafterName
        let when = Ago.text(from: item.request.createdAt.date, now: now)
        return [Ja.Desk.drafter(drafter), item.teamLabel, when].joined(separator: " · ")
    }

    /// Whom the viewer answers it for, when it is addressed to a seat or an external party.
    static func answerLine(of item: DeskItem) -> String? {
        item.answersFor.map(Ja.Desk.answerFor)
    }
}
#endif
