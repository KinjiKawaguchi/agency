#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// How a movement is drawn: a symbol and its words, coloured by whether it is the better way. The words say the same
/// as the colour, so nothing depends on seeing it.
struct MovementLine: View {
    let movement: Movement

    private var symbol: String {
        switch movement.direction {
        case .rose: "arrow.up.right"
        case .fell: "arrow.down.right"
        case .unchanged: "equal"
        }
    }

    private var color: Color {
        switch movement.tone {
        case .better: .green
        case .worse: Theme.danger
        case .neutral: .secondary
        }
    }

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            Image(systemName: symbol).accessibilityHidden(true)
            Text(movement.text).wrapping()
        }
        .font(.subheadline.weight(.semibold))
        .foregroundStyle(color)
    }
}

/// One KPI: its label, the latest number by its unit, how it moved, and where its numbers sit in the period.
struct KpiCardView: View {
    let card: KpiCard
    let from: String
    let until: String

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(card.label).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary).wrapping()
            figure
            if let movement = card.movement { MovementLine(movement: movement) }
            if !card.points.isEmpty {
                TrendView(layout: TrendLayout(points: card.points, from: from, until: until))
            }
        }
        .card()
        .accessibilityElement(children: .combine)
    }

    @ViewBuilder
    private var figure: some View {
        switch card.figure {
        case .latest(let text, let day):
            Text(text).font(.title.weight(.bold)).lineLimit(2).minimumScaleFactor(0.6).wrapping()
            Text(Ja.Outcomes.lastEntered(day)).font(.caption).foregroundStyle(.secondary).wrapping()
        case .older(let text, let day):
            gap
            Text(Ja.Outcomes.lastNumber(text, day: day)).font(.caption).foregroundStyle(.secondary).wrapping()
        case .none:
            gap
        }
    }

    /// No number in the period: a dash and what it is, never a 0.
    @ViewBuilder
    private var gap: some View {
        Text(Ja.Outcomes.gap).font(.title.weight(.bold)).foregroundStyle(.secondary)
        Text(Ja.Outcomes.noNumber).font(.caption).foregroundStyle(.secondary).wrapping()
    }
}
#endif
