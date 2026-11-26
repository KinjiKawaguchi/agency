#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// What each participant of a team did over the period: how their time went (a bar of three parts, working, waiting and idle,
/// by their share of the three, and the three durations in words, so that nothing depends on the colours), what they cost, and the
/// pull requests merged under their name. The same as the web's section, drawn after the team's charts.
struct ParticipantsView: View {
    let section: ParticipantsSection

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            Text(Ja.Evaluation.Participants.title).font(.title3.weight(.semibold)).wrapping()
            Text(Ja.Evaluation.Participants.lead).font(.footnote).foregroundStyle(.secondary).wrapping()
            Text(Ja.Evaluation.Participants.time).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary).wrapping()
            Text(Ja.Evaluation.Participants.timeNote).font(.caption).foregroundStyle(.secondary).wrapping()
            ForEach(section.rows) { ParticipantCard(row: $0) }
            if let unattributed = section.unattributed {
                Text(unattributed).font(.footnote).foregroundStyle(.secondary).wrapping()
            }
        }
    }
}

private struct ParticipantCard: View {
    let row: ParticipantsSection.Row

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(row.name).font(.subheadline.weight(.semibold)).wrapping()
            if row.hasSession {
                TimeBar(parts: row.times).accessibilityLabel(row.timesSpoken)
                ForEach(row.times) { part in
                    HStack(alignment: .firstTextBaseline, spacing: 6) {
                        Circle().fill(TimeBar.color(part.kind)).frame(width: 8, height: 8).accessibilityHidden(true)
                        Text(part.label).font(.caption).wrapping()
                        Text(part.duration).font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                    }
                }
            } else {
                Text(Ja.Evaluation.Participants.noSession).font(.caption).foregroundStyle(.secondary).wrapping()
            }
            line(Ja.Evaluation.Participants.cost, "\(row.cost)・\(row.tokens)")
            line(Ja.Evaluation.Participants.merged, row.merged).accessibilityLabel(row.mergedSpoken)
            line(Ja.Evaluation.Participants.perPullRequest, row.costPerPullRequest)
        }
        .card()
    }

    private func line(_ label: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(label).font(.caption).foregroundStyle(.secondary).wrapping()
            Text(value).font(.caption.weight(.semibold).monospacedDigit()).multilineTextAlignment(.trailing)
        }
    }
}

/// The three parts of a participant's time side by side, each as wide as its share of the three.
private struct TimeBar: View {
    let parts: [ParticipantsSection.TimePart]

    static func color(_ kind: ParticipantsSection.TimePart.Kind) -> Color {
        switch kind {
        case .working: .accentColor
        case .waiting: Theme.warning
        case .idle: Color.secondary.opacity(0.4)
        }
    }

    var body: some View {
        GeometryReader { proxy in
            HStack(spacing: 0) {
                ForEach(parts) { part in
                    Rectangle().fill(Self.color(part.kind)).frame(width: proxy.size.width * part.share)
                }
            }
        }
        .frame(height: 10)
        .clipShape(Capsule())
    }
}
#endif
