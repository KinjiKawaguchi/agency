import AgencyAPI
import Foundation

/// A KPI's numbers written by its unit. Money keeps its own currency, whatever it is, and is never converted; a percent is
/// the number as it is (it may be above 100 or below 0, and is never clipped); a duration is written as the evaluation
/// writes one. A number of a unit this app does not know is written as a plain number.
public enum OutcomeFormat {
    private static let locale = Locale(identifier: "ja_JP")

    private static func plain(_ value: Double, fractionDigits: Int, signed: Bool = false) -> String {
        let formatter = NumberFormatter()
        formatter.locale = locale
        formatter.numberStyle = .decimal
        formatter.minimumFractionDigits = 0
        formatter.maximumFractionDigits = fractionDigits
        formatter.usesGroupingSeparator = true
        if signed {
            formatter.positivePrefix = "+"
            formatter.negativePrefix = "-"
        }
        return formatter.string(from: NSNumber(value: value)) ?? "\(value)"
    }

    private static func money(_ value: Double, currency: String, signed: Bool) -> String {
        let formatter = NumberFormatter()
        formatter.locale = locale
        formatter.numberStyle = .currency
        formatter.currencyCode = currency
        if signed {
            let symbol = formatter.currencySymbol ?? currency
            formatter.positivePrefix = "+" + symbol
            formatter.negativePrefix = "-" + symbol
        }
        return formatter.string(from: NSNumber(value: value)) ?? "\(value) \(currency)"
    }

    /// A number as the KPI's unit writes it.
    public static func value(_ value: Double, unit: Kpi.Unit) -> String {
        switch unit {
        case .money(let currency): money(value, currency: currency, signed: false)
        case .percent: plain(value, fractionDigits: 2) + "%"
        case .count, .unknown: plain(value, fractionDigits: 2)
        case .ratio: plain(value, fractionDigits: 3) + "×"
        case .seconds: (value < 0 ? "-" : "") + DurationFormat.text(abs(value))
        }
    }

    /// A change between two numbers of the unit, with its sign (`+4ポイント` for a percent, `+¥150,000` for money).
    public static func change(_ change: Double, unit: Kpi.Unit) -> String {
        guard change != 0 else { return value(0, unit: unit) }
        switch unit {
        case .money(let currency): return money(change, currency: currency, signed: true)
        case .percent: return plain(change, fractionDigits: 2, signed: true) + Ja.Outcomes.points
        case .count, .unknown: return plain(change, fractionDigits: 2, signed: true)
        case .ratio: return plain(change, fractionDigits: 3, signed: true) + "×"
        case .seconds: return (change < 0 ? "-" : "+") + DurationFormat.text(abs(change))
        }
    }

    /// A day `YYYY-MM-DD` as it is read in Japanese (`2026年11月20日`); anything else as it came.
    public static func day(_ iso: String) -> String {
        let parts = iso.split(separator: "-")
        guard parts.count == 3, let year = Int(parts[0]), let month = Int(parts[1]), let day = Int(parts[2]) else { return iso }
        return "\(year)年\(month)月\(day)日"
    }
}
