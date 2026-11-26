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
    /// "上がった（良い方）"; the direction alone when the KPI does not say which way is better.
    public let text: String

    /// The movement from `from` to `to`. `goodWhen` says which way is better; an unknown one colours nothing.
    public init(from: Double, to: Double, unit: Kpi.Unit, goodWhen: Kpi.GoodWhen) {
        let delta = to - from
        direction = delta > 0 ? .rose : delta < 0 ? .fell : .unchanged
        tone = switch (direction, goodWhen) {
        case (.unchanged, _): .neutral
        case (.rose, .higher), (.fell, .lower): .better
        case (.rose, .lower), (.fell, .higher): .worse
        case (_, .unknown): .neutral
        }
        change = OutcomeFormat.change(delta, unit: unit)
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
        text = Ja.Outcomes.movement(word, tone: toneWord)
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

    public init(series: KpiSeries) {
        let kpi = series.kpi
        id = kpi.id
        label = kpi.label
        unit = kpi.unit
        points = series.points.map { Point(day: $0.date, value: $0.value) }
        if let latest = series.points.last {
            figure = .latest(text: OutcomeFormat.value(latest.value, unit: kpi.unit), day: OutcomeFormat.day(latest.date))
            movement = series.before.map { Movement(from: $0.value, to: latest.value, unit: kpi.unit, goodWhen: kpi.goodWhen) }
        } else if let before = series.before {
            figure = .older(text: OutcomeFormat.value(before.value, unit: kpi.unit), day: OutcomeFormat.day(before.date))
            movement = nil
        } else {
            figure = .none
            movement = nil
        }
    }
}
