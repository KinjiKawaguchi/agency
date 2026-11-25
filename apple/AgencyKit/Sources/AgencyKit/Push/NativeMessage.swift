import Foundation

/// A message the page sends to the app through `window.agencyNative`.
///
/// The page never sees the message handler: it calls `window.agencyNative.setBadge(count)`, and the
/// script `NativeBridge` writes puts `{ type: "badge", count }` on the handler. The app treats every
/// message as untrusted input: anything that is not exactly a known message gives `nil`.
public enum NativeMessage: Equatable, Sendable {
    /// The number of requests waiting for the owner, to show on the app icon.
    case badge(Int)

    /// The name of the script message handler the app registers.
    public static let handlerName = "agency"

    public static let badgeRange = 0...9999

    public init?(body: Any) {
        guard let fields = body as? [String: Any], let type = fields["type"] as? String else { return nil }
        switch type {
        case "badge":
            guard let count = Self.wholeNumber(fields["count"]), Self.badgeRange.contains(count) else { return nil }
            self = .badge(count)
        default:
            return nil
        }
    }

    private static func wholeNumber(_ value: Any?) -> Int? {
        guard let value, !isBoolean(value) else { return nil }
        if let integer = value as? Int { return integer }
        if let number = value as? Double, abs(number) < 1_000_000, number == number.rounded(.towardZero) {
            return Int(number)
        }
        return nil
    }

    /// WebKit hands a JavaScript `true` over as an `NSNumber` that casts to the integer 1.
    private static func isBoolean(_ value: Any) -> Bool {
        #if canImport(Darwin)
        guard let number = value as? NSNumber else { return false }
        return CFGetTypeID(number) == CFBooleanGetTypeID()
        #else
        return value is Bool
        #endif
    }
}
