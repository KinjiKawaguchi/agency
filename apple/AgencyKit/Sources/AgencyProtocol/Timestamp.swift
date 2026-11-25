import Foundation

/// Times in the protocol are RFC 3339 in UTC and end in `Z`.
public enum Timestamp {
    /// `2026-11-24T12:00:00Z`: whole seconds.
    public static func string(from date: Date) -> String {
        date.formatted(Date.ISO8601FormatStyle(timeZone: .gmt))
    }

    /// Reads an RFC 3339 time, with or without fractional seconds. Anything else is `nil`.
    public static func date(from text: String) -> Date? {
        let plain = try? Date.ISO8601FormatStyle(timeZone: .gmt).parse(text)
        if let plain { return plain }
        return try? Date.ISO8601FormatStyle(includingFractionalSeconds: true, timeZone: .gmt).parse(text)
    }
}
