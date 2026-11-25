#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// The last decisions, with where each went. A decision that waits to be sent says so.
struct RecentDecisions: View {
    let entries: [DecisionEntry]
    let failure: String?
    let now: Date

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(Ja.Desk.recentHead).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary).wrapping()
            if let failure {
                Text(Ja.Recent.unavailable(failure)).font(.footnote).foregroundStyle(Theme.danger).wrapping()
            }
            ForEach(entries, id: \.id) { entry in
                VStack(alignment: .leading, spacing: 4) {
                    HStack(spacing: 8) {
                        Tag(text: Ja.Recent.state(entry.state), color: color(of: entry.state))
                        Text(Ago.text(from: entry.decidedAt.date, now: now)).font(.caption).foregroundStyle(.secondary)
                    }
                    Text(entry.title ?? entry.target).font(.subheadline.weight(.semibold)).lineLimit(2).wrapping()
                    if let line = summary(entry) { Text(line).font(.footnote).lineLimit(3).wrapping() }
                    if let detail = detail(entry.state, now: now) {
                        Text(detail).font(.caption).foregroundStyle(.secondary).wrapping()
                    }
                }
                .card()
            }
        }
    }

    private func summary(_ entry: DecisionEntry) -> String? {
        if let label = entry.choiceLabel ?? entry.choice {
            return [Ja.Recent.chose(label), entry.text].filter { !$0.isEmpty }.joined(separator: " ")
        }
        return entry.text.isEmpty ? nil : "\(Ja.Recent.commented) \(entry.text)"
    }

    private func detail(_ state: DecisionState, now: Date) -> String? {
        switch state {
        case .queued(let attempts, let nextAt, let lastError):
            let retry = nextAt.date > now ? Ago.text(from: nextAt.date, now: now) : nil
            return [Ja.Recent.queuedDetail(attempts: attempts, retry: retry), lastError.map(Ja.Recent.lastError)].compactMap { $0 }.joined(separator: " ")
        case .refused(let message):
            return Ja.Recent.refusedDetail(message)
        case .applied, .delivered:
            return nil
        }
    }

    private func color(of state: DecisionState) -> Color {
        switch state {
        case .applied, .delivered: .green
        case .queued: Theme.warning
        case .refused: Theme.danger
        }
    }
}
#endif
