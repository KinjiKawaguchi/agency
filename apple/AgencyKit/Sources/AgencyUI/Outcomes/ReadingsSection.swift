#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// What an org's aims came to: for each, how its KPI stood around the day the epic shipped, in the words of its state.
struct ReadingsSection: View {
    let readings: OrgReadings
    var onTryAgain: (() -> Void)?

    var body: some View {
        switch readings {
        case .unavailable:
            EmptyView()
        case .failed(let reason):
            FailedRegion(what: Ja.Outcomes.readingsHeading, why: reason, onTryAgain: onTryAgain)
        case .lines(let lines):
            VStack(alignment: .leading, spacing: Theme.spacing) {
                Text(Ja.Outcomes.readingsHeading).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary).wrapping()
                if lines.isEmpty { Text(Ja.Outcomes.noAims).font(.subheadline).foregroundStyle(.secondary).wrapping() }
                ForEach(lines) { ReadingCard(line: $0) }
            }
        }
    }
}

private struct ReadingCard: View {
    let line: ReadingLine

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("\(line.team)・\(line.epic)").font(.subheadline.weight(.semibold)).wrapping()
            Text(line.kpi).font(.caption).foregroundStyle(.secondary).wrapping()
            Text(line.headline).font(.subheadline).wrapping()
            if let movement = line.movement { MovementLine(movement: movement) }
            if let note = line.note { Text(note).font(.caption).foregroundStyle(.secondary).wrapping() }
            if let overlap = line.overlap { Text(overlap).font(.caption).foregroundStyle(.secondary).wrapping() }
        }
        .card()
    }
}
#endif
