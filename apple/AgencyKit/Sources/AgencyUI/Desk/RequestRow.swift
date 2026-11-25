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
            Text(meta)
                .font(.footnote)
                .foregroundStyle(.secondary)
                .lineLimit(2)
                .wrapping()
        }
        .card()
        .accessibilityElement(children: .combine)
    }

    private var meta: String {
        let drafter = item.drafter?.label ?? item.request.from
        let when = Ago.text(from: item.request.createdAt.date, now: now)
        return "\(Ja.Desk.drafter(drafter)) · \(item.teamLabel) · \(when)"
    }
}
#endif
