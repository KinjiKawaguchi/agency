#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// One team on the overview: what it needs from the viewer, who is working, what is stuck, and the week's cost.
struct TeamCard: View {
    let entry: TeamEntry
    let cost: WeekCost
    let now: Date
    /// Opens the desk. `nil` draws the line without a button, for renderings.
    var onOpenDesk: (() -> Void)?

    private var summary: TeamSummary? { entry.summary }

    private var stripe: Color {
        switch (entry.state, summary?.attention) {
        case (.failed, _): .secondary
        case (_, .mine?): .accentColor
        case (_, .stuck?): Theme.warning
        default: .clear
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            header
            switch entry.state {
            case .loading:
                Text(Ja.Desk.loading).font(.subheadline).foregroundStyle(.secondary).wrapping()
            case .failed(let reason, let signInEnded):
                Text(signInEnded ? Ja.Overview.signInNothingToShow : Ja.Overview.loadFailed(reason))
                    .font(.subheadline).foregroundStyle(.secondary).wrapping()
            case .ready(let team):
                if let summary { ReadyBody(team: team, summary: summary, cost: cost, now: now, onOpenDesk: onOpenDesk) }
            }
        }
        .padding(Theme.spacing)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.card, in: RoundedRectangle(cornerRadius: Theme.corner))
        .overlay(alignment: .leading) {
            Rectangle().fill(stripe).frame(width: 4).clipShape(RoundedRectangle(cornerRadius: 2)).padding(.vertical, 1)
        }
        .overlay(RoundedRectangle(cornerRadius: Theme.corner).stroke(Theme.hairline, style: StrokeStyle(lineWidth: 1, dash: isFailed ? [4, 3] : [])))
        .accessibilityElement(children: .contain)
    }

    private var isFailed: Bool {
        if case .failed = entry.state { true } else { false }
    }

    private var header: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            ExpandableTitle(text: entry.ref.label)
            Spacer(minLength: 0)
            if let tag {
                Tag(text: tag.text, color: tag.color)
            }
        }
    }

    private var tag: (text: String, color: Color)? {
        if isFailed { return (Ja.Overview.failedTag, .secondary) }
        guard let attention = summary?.attention, let text = Ja.Overview.tag(attention) else { return nil }
        return (text, attention == .mine ? .accentColor : attention == .stuck ? Theme.warning : .secondary)
    }
}

private struct ReadyBody: View {
    let team: Team
    let summary: TeamSummary
    let cost: WeekCost
    let now: Date
    let onOpenDesk: (() -> Void)?

    var body: some View {
        if let age = DataAge(team: team, now: now) {
            Text(age.text).font(.footnote).foregroundStyle(age.isStale ? Theme.warning : .secondary).wrapping()
        }
        waiting
        Text(summary.participants == 0 ? Ja.Overview.noParticipants : Ja.Overview.working(summary.working, of: summary.participants))
            .font(.subheadline).wrapping()
        if summary.blockedCount > 0 {
            reasons(Ja.Overview.blocked(summary.blockedCount), summary.blocked.map { "\(Ja.Work.blockedReason($0.reason)) (\($0.count))" })
        }
        if summary.attention == .idle {
            reasons(Ja.Overview.nobodyWorking, summary.quiet)
        }
        usage
    }

    @ViewBuilder
    private var waiting: some View {
        if summary.mine > 0 {
            if let onOpenDesk {
                Button(action: onOpenDesk) { Label(Ja.Overview.waitingForYou(summary.mine), systemImage: "tray.full").wrapping() }
                    .font(.subheadline.weight(.semibold))
            } else {
                Label(Ja.Overview.waitingForYou(summary.mine), systemImage: "tray.full").font(.subheadline.weight(.semibold)).wrapping()
            }
        } else {
            Text(Ja.Overview.nothingWaits).font(.subheadline).foregroundStyle(.secondary).wrapping()
        }
    }

    private func reasons(_ head: String, _ lines: [String]) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(head).font(.subheadline.weight(.semibold)).wrapping()
            ForEach(lines, id: \.self) { line in
                Text("・\(line)").font(.footnote).foregroundStyle(.secondary).wrapping()
            }
        }
        .padding(.top, 2)
    }

    @ViewBuilder
    private var usage: some View {
        switch cost {
        case .loading: EmptyView()
        case .known(let amount): Text(Ja.Overview.usage(JaFormat.money(amount))).font(.footnote).wrapping()
        case .none: Text(Ja.Overview.usageNone).font(.footnote).foregroundStyle(.secondary).wrapping()
        case .failed: Text(Ja.Overview.usageFailed).font(.footnote).foregroundStyle(.secondary).wrapping()
        }
    }
}
#endif
