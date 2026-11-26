import Foundation

/// A span of time as the web client writes it in Japanese: 分 under an hour, 時間 under two days, 日 above.
public enum DurationFormat {
    /// One decimal under ten, none above, and no trailing ".0": 0.5, 1.2, 12.
    static func tidy(_ value: Double) -> String {
        if value >= 10 { return String(Int(value.rounded())) }
        let tenths = (value * 10).rounded() / 10
        return tenths == tenths.rounded() ? String(Int(tenths)) : String(tenths)
    }

    public static func text(_ seconds: Double) -> String {
        if seconds < EvaluationSeries.secondsPerHour { return "\(Int((seconds / EvaluationSeries.secondsPerMinute).rounded()))分" }
        if seconds < 2 * EvaluationSeries.secondsPerDay { return tidy(seconds / EvaluationSeries.secondsPerHour) + "時間" }
        return tidy(seconds / EvaluationSeries.secondsPerDay) + "日"
    }

    /// An axis for spans: its top is a round number of one unit (minutes, hours or days), and every tick is written in that unit.
    public struct Axis: Equatable, Sendable {
        /// The top of the axis, in seconds.
        public let max: Double
        let size: Double
        let label: String

        public func text(_ seconds: Double) -> String { tidy(seconds / size) + label }
    }

    public static func axis(upTo maxSeconds: Double) -> Axis {
        let unit: (size: Double, label: String) =
            maxSeconds < EvaluationSeries.secondsPerHour ? (EvaluationSeries.secondsPerMinute, "分")
            : maxSeconds <= 2 * EvaluationSeries.secondsPerDay ? (EvaluationSeries.secondsPerHour, "時間")
            : (EvaluationSeries.secondsPerDay, "日")
        return Axis(max: EvaluationSeries.niceMax(maxSeconds / unit.size) * unit.size, size: unit.size, label: unit.label)
    }
}
