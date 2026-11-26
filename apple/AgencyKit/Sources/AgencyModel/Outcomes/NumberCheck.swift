import AgencyAPI
import Foundation

/// What the owner types to enter a number of a KPI.
public struct NumberForm: Equatable, Sendable {
    public var date: String
    public var value: String
    public var note: String

    public init(date: String, value: String = "", note: String = "") {
        self.date = date
        self.value = value
        self.note = note
    }
}

/// What is wrong with what was typed, said before it is sent.
public enum NumberProblem: Error, Equatable, Sendable {
    case date, value, note

    public var text: String {
        switch self {
        case .date: Ja.Outcomes.Enter.problemDate
        case .value: Ja.Outcomes.Enter.problemValue
        case .note: Ja.Outcomes.Enter.problemNote
        }
    }
}

/// What the owner typed, checked as the node checks it (docs/protocol.md, "Outcomes"), so that a mistake is said before it is sent:
/// a real calendar day that is not more than one day ahead of the node's today and not older than ten years; a finite number of at
/// most 1e15 written as digits, a sign and a point (never `0x1F`, `1e3` or `inf`, which a figure typed here does not mean); and a
/// note of at most 200 characters with no control character. The node still decides: this only saves the round trip.
public enum NumberCheck {
    public static let largestValue = 1e15
    public static let longestNote = 200

    public static func check(_ form: NumberForm, kpi: String, today: String) -> Result<ObservationInput, NumberProblem> {
        guard let day = days(form.date), let now = days(today), day <= now + 1, day >= tenYearsBefore(today) else {
            return .failure(.date)
        }
        let written = form.value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard isDecimal(written), let value = Double(written), value.isFinite, abs(value) <= largestValue else { return .failure(.value) }
        let note = form.note.trimmingCharacters(in: .whitespacesAndNewlines)
        // The node counts characters (Unicode scalars), not bytes and not grapheme clusters.
        guard note.unicodeScalars.count <= longestNote, !note.unicodeScalars.contains(where: { $0.value < 0x20 || $0.value == 0x7F }) else {
            return .failure(.note)
        }
        return .success(ObservationInput(kpi: kpi, date: form.date, value: value, note: note.isEmpty ? nil : note))
    }

    /// `[+-]?(digits[.digits?] | .digits)` in ASCII digits: nothing else is a figure.
    static func isDecimal(_ text: String) -> Bool {
        var rest = Substring(text)
        if rest.first == "+" || rest.first == "-" { rest = rest.dropFirst() }
        let parts = rest.split(separator: ".", maxSplits: 1, omittingEmptySubsequences: false)
        guard parts.count <= 2, !parts.isEmpty else { return false }
        let isAsciiDigits: (Substring) -> Bool = { $0.unicodeScalars.allSatisfy { $0.value >= 0x30 && $0.value <= 0x39 } }
        guard isAsciiDigits(parts[0]), parts.count == 1 || isAsciiDigits(parts[1]) else { return false }
        return !parts[0].isEmpty || (parts.count == 2 && !parts[1].isEmpty)
    }

    /// A day `YYYY-MM-DD` (exactly four, two and two digits, and a day that exists) as a whole number of days; none otherwise.
    static func days(_ text: String) -> Int? {
        let parts = text.split(separator: "-", omittingEmptySubsequences: false)
        guard parts.count == 3, parts[0].count == 4, parts[1].count == 2, parts[2].count == 2,
              parts.allSatisfy({ $0.unicodeScalars.allSatisfy { $0.value >= 0x30 && $0.value <= 0x39 } }),
              let year = Int(parts[0]), let month = Int(parts[1]), let day = Int(parts[2])
        else { return nil }
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "UTC") ?? .gmt
        guard let date = calendar.date(from: DateComponents(year: year, month: month, day: day)) else { return nil }
        let back = calendar.dateComponents([.year, .month, .day], from: date)
        guard back.year == year, back.month == month, back.day == day else { return nil }
        return Int((date.timeIntervalSince1970 / 86_400).rounded(.down))
    }

    /// The first day the node still takes: the same month and day ten years before today (a February 29 becomes March 1, as in
    /// the web's check).
    static func tenYearsBefore(_ today: String) -> Int {
        let parts = today.split(separator: "-").compactMap { Int($0) }
        guard parts.count == 3 else { return Int.max }
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "UTC") ?? .gmt
        guard let date = calendar.date(from: DateComponents(year: parts[0] - 10, month: parts[1], day: parts[2])) else { return Int.max }
        return Int((date.timeIntervalSince1970 / 86_400).rounded(.down))
    }
}
