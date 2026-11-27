#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// The decisions the owner has made that the node has not yet taken: what is in flight, and each failure with its reason, "send again"
/// (the same signed decision) and "discard" (the item returns to the pile). It does not block anything: the owner is already on the next
/// item. It draws nothing when nothing is kept.
struct SendingTray: View {
    let entries: [PendingDecision]
    let sending: Set<String>
    let needsSignIn: Bool
    let onSendAgain: (String) -> Void
    let onDiscard: (String) -> Void

    var body: some View {
        if !entries.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Label {
                    Text(Ja.Outbox.heading(count: entries.count)).font(.subheadline.weight(.semibold))
                } icon: {
                    Image(systemName: "paperplane")
                }
                if needsSignIn {
                    Text(Ja.Outbox.signInEnded).font(.footnote).foregroundStyle(Theme.warning).wrapping()
                }
                ForEach(entries.prefix(Ja.Outbox.shown)) { entry in row(entry) }
                if entries.count > Ja.Outbox.shown {
                    Text(Ja.Outbox.more(entries.count - Ja.Outbox.shown)).font(.footnote).foregroundStyle(.secondary)
                }
            }
            .card()
        }
    }

    private func row(_ entry: PendingDecision) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(entry.title ?? entry.target).font(.subheadline).lineLimit(2).wrapping()
            if let line = chosen(entry) { Text(line).font(.caption).foregroundStyle(.secondary).wrapping() }
            state(of: entry)
        }
        .accessibilityElement(children: .combine)
    }

    /// What was chosen, in the decision log's own sentence, or that it is a comment.
    private func chosen(_ entry: PendingDecision) -> String? {
        if entry.isComment { return Ja.Recent.commented }
        return entry.choiceLabel.map(Ja.Recent.chose)
    }

    @ViewBuilder
    private func state(of entry: PendingDecision) -> some View {
        if sending.contains(entry.id) || entry.failure == nil {
            HStack(spacing: 6) { ProgressView().controlSize(.small); Text(Ja.Outbox.sending).font(.footnote) }
        } else if let failure = entry.failure {
            VStack(alignment: .leading, spacing: 6) {
                if failure.isFinal {
                    Text(Ja.Outbox.failed(failure.reason)).font(.footnote).foregroundStyle(Theme.danger).wrapping()
                } else {
                    // A failure that is tried again has the time of its next try.
                    if let next = entry.nextAttemptAt {
                        Text(Ja.Outbox.waiting(nextInSeconds: Int(next.timeIntervalSinceNow.rounded(.up))))
                            .font(.footnote).foregroundStyle(Theme.warning).wrapping()
                    }
                    if !failure.reason.isEmpty { Text(failure.reason).font(.caption).foregroundStyle(.secondary).wrapping() }
                    Text(Ja.Outbox.attempts(entry.attempts)).font(.caption).foregroundStyle(.secondary)
                }
                HStack(spacing: 8) {
                    Button(Ja.Outbox.sendAgain) { onSendAgain(entry.id) }.buttonStyle(.bordered)
                    // Only a decision that stopped can be let go of: one that is waiting to be tried again may be taken any moment.
                    if failure.isFinal { Button(Ja.Outbox.discard, role: .destructive) { onDiscard(entry.id) }.buttonStyle(.bordered) }
                }
                .font(.footnote)
                if failure.isFinal { Text(Ja.Outbox.discardExplain).font(.caption).foregroundStyle(.secondary).wrapping() }
            }
        }
    }
}
#endif
