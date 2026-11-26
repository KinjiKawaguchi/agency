import Foundation

/// Numbers and times as the web client writes them in Japanese.
public enum JaFormat {
    /// `62%` from 62.
    public static func percent(_ value: Double) -> String { "\(Int(value.rounded()))%" }

    /// `1.2K`, `3.4M`, `5.67B`; a plain number below a thousand.
    public static func compact(_ value: Int) -> String {
        let number = Double(value)
        if number >= 1e9 { return String(format: "%.2fB", number / 1e9) }
        if number >= 1e6 { return String(format: "%.1fM", number / 1e6) }
        if number >= 1e3 { return String(format: "%.1fK", number / 1e3) }
        return String(value)
    }

    /// US dollars: cents under 10, whole dollars up to 1000, then grouped thousands.
    public static func money(_ usd: Double) -> String {
        if usd >= 1000 { return "$" + (grouped(Int(usd.rounded()))) }
        return "$" + String(format: usd >= 10 ? "%.0f" : "%.2f", usd)
    }

    /// `11月25日 15:00`.
    public static func dateTime(_ date: Date, in timeZone: TimeZone = .current) -> String {
        formatter("M月d日 H:mm", timeZone).string(from: date)
    }

    /// `11月25日` from `2026-11-25`; the text as it is when it is not a date.
    public static func shortDate(_ day: String) -> String {
        let parts = day.split(separator: "-").compactMap { Int($0) }
        guard parts.count == 3 else { return day }
        return "\(parts[1])月\(parts[2])日"
    }

    private static func grouped(_ value: Int) -> String {
        let digits = String(value)
        return digits.enumerated().map { index, digit in
            (digits.count - index).isMultiple(of: 3) && index > 0 ? ",\(digit)" : "\(digit)"
        }.joined()
    }

    private static func formatter(_ format: String, _ timeZone: TimeZone) -> DateFormatter {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "ja_JP")
        formatter.timeZone = timeZone
        formatter.dateFormat = format
        return formatter
    }
}
