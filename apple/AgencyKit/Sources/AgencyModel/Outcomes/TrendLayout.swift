import Foundation

/// Where the numbers of a KPI sit in a small plot of its period: across by the day each is as of, so that a day without a
/// number is a gap and not a step, and up by its value between the lowest and the highest of them. Nothing is drawn for a
/// day with no number, and nothing is invented between two numbers but the line that joins them.
public struct TrendLayout: Equatable, Sendable {
    public struct Spot: Equatable, Sendable {
        /// 0 at the period's first day, 1 at its last.
        public let x: Double
        /// 0 at the bottom, 1 at the top; a series whose numbers are all the same sits in the middle.
        public let y: Double
    }

    public let spots: [Spot]

    public init(points: [KpiCard.Point], from: String, until: String) {
        let first = OutcomeDay.number(from)
        let last = OutcomeDay.number(until)
        let values = points.map(\.value)
        let low = values.min()
        let high = values.max()
        spots = points.compactMap { point in
            guard let first, let last, let day = OutcomeDay.number(point.day), let low, let high else { return nil }
            let span = Double(last - first)
            let across = span > 0 ? Double(day - first) / span : 0.5
            let up = high > low ? (point.value - low) / (high - low) : 0.5
            return Spot(x: min(max(across, 0), 1), y: up)
        }
    }
}

/// A calendar day `YYYY-MM-DD` as a whole number of days, for the distance between two of them.
enum OutcomeDay {
    static func number(_ iso: String) -> Int? {
        let parts = iso.split(separator: "-")
        guard parts.count == 3, let year = Int(parts[0]), let month = Int(parts[1]), let day = Int(parts[2]) else { return nil }
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "UTC") ?? .gmt
        guard let date = calendar.date(from: DateComponents(year: year, month: month, day: day)) else { return nil }
        return Int((date.timeIntervalSince1970 / 86_400).rounded(.down))
    }
}
