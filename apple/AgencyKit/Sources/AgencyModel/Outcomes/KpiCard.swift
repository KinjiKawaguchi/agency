import AgencyAPI
import Foundation

/// How a number moved, and whether that is the better way. The colour of a screen follows `tone`, and a word says the
/// same, so the movement does not depend on colour.
public struct Movement: Equatable, Sendable {
    public enum Direction: Equatable, Sendable { case rose, fell, unchanged }
    public enum Tone: Equatable, Sendable { case better, worse, neutral }

    public let direction: Direction
    public let tone: Tone
    /// The change, signed and in the unit's words.
    public let change: String
    /// The change as a share of where it began (`+10%`); none when it began at 0.
    public let ratio: String?
    /// "上がった +4ポイント（+10%）良い方": the direction, the change, its share, and whether it is the better way when the KPI
    /// says which is.
    public let text: String

    /// The movement from `from` to `to`. `goodWhen` says which way is better; an unknown one colours nothing.
    public init(from: Double, to: Double, unit: Kpi.Unit, goodWhen: Kpi.GoodWhen) {
        let delta = to - from
        // The direction is the one the written change says: a change that is written as nothing (+0) is not a movement.
        direction = OutcomeFormat.isNothing(OutcomeFormat.change(delta, unit: unit)) ? .unchanged : delta > 0 ? .rose : .fell
        tone = switch (direction, goodWhen) {
        case (.unchanged, _): .neutral
        case (.rose, .higher), (.fell, .lower): .better
        case (.rose, .lower), (.fell, .higher): .worse
        case (_, .unknown): .neutral
        }
        change = OutcomeFormat.change(delta, unit: unit)
        ratio = direction == .unchanged ? nil : OutcomeFormat.changeRatio(from: from, to: to)
        let word = switch direction {
        case .rose: Ja.Outcomes.rose
        case .fell: Ja.Outcomes.fell
        case .unchanged: Ja.Outcomes.unchanged
        }
        let toneWord: String? = switch tone {
        case .better: Ja.Outcomes.better
        case .worse: Ja.Outcomes.worse
        case .neutral: nil
        }
        text = Ja.Outcomes.movement(word, change: direction == .unchanged ? nil : change, ratio: ratio, tone: toneWord)
    }
}

/// One KPI as the page draws it: its latest number, how it moved, and the numbers of the period.
public struct KpiCard: Equatable, Identifiable, Sendable {
    /// What the card says of the number.
    public enum Figure: Equatable, Sendable {
        /// A number of the period: the last one, and the day it is as of.
        case latest(text: String, day: String)
        /// No number in the period, but one before it: shown as the last number, older than the period.
        case older(text: String, day: String)
        /// No number at all: a dash, never 0.
        case none
    }

    public struct Point: Equatable, Sendable {
        public let day: String
        public let value: Double
    }

    public let id: String
    public let label: String
    public let figure: Figure
    /// From the baseline (`before`) to the latest number; none without both.
    public let movement: Movement?
    /// The numbers of the period, oldest first, each on its own day: a day without a number has no point, never a 0.
    public let points: [Point]
    public let unit: Kpi.Unit
    /// The KPI itself, which entering a number for it needs.
    public let kpi: Kpi

    /// - Parameter until: the period's last day, which a day in the same year is written without its year.
    public init(series: KpiSeries, until: String? = nil) {
        let kpi = series.kpi
        self.kpi = kpi
        id = kpi.id
        label = kpi.label
        unit = kpi.unit
        // The node sends them oldest first, but the card does not rely on it: the latest is the one of the latest day.
        let ordered = series.points.sorted { $0.date < $1.date }
        points = ordered.map { Point(day: $0.date, value: $0.value) }
        if let latest = ordered.last {
            figure = .latest(text: OutcomeFormat.value(latest.value, unit: kpi.unit), day: OutcomeFormat.day(latest.date, relativeTo: until))
            movement = series.before.map { Movement(from: $0.value, to: latest.value, unit: kpi.unit, goodWhen: kpi.goodWhen) }
        } else if let before = series.before {
            figure = .older(text: OutcomeFormat.value(before.value, unit: kpi.unit), day: OutcomeFormat.day(before.date, relativeTo: until))
            movement = nil
        } else {
            figure = .none
            movement = nil
        }
    }
}
