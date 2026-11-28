import Foundation

/// The words of the notices the node writes in English for a recovery that was started and for a change held for the root key's
/// signature (`docs/protocol.md`, "Push notifications"), put in the viewer's language with its times in the viewer's zone, from the
/// facts the payload carries: `agency: { type: "recovery", org, startedAt?, finishesAt? }` and `agency: { type: "root", org, id,
/// waiting }`. The node's own `title` and `body` are for a client that does not know the kind; they name a member's or a device's
/// label, which these words leave out. The same words as the web's worker (`push-notice.js`), so that one event reads the same on
/// every surface.
public enum PushNotice {
    /// The title of such a notice: the app's name, as the web's.
    public static let title = "Agency"

    /// The body for a notice of the kind `recovery` or `root`; `nil` for any other (a request, a device and a refused decision are
    /// worded by the system from the payload's `loc-key`).
    ///
    /// - Parameters:
    ///   - language: the viewer's language (`Locale.preferredLanguages.first`): Japanese when it starts with `ja`, else English.
    ///   - timeZone: the viewer's zone, for the times a recovery names.
    public static func body(userInfo: [AnyHashable: Any], language: String, timeZone: TimeZone) -> String? {
        guard let keys = userInfo["agency"] as? [String: Any], let type = keys["type"] as? String else { return nil }
        let isJapanese = language.lowercased().hasPrefix("ja")
        let org = RequestLink.identifier(keys["org"])
        switch type {
        case "recovery":
            let locale = Locale(identifier: isJapanese ? "ja_JP" : "en_US")
            let started = time(keys["startedAt"], locale: locale, timeZone: timeZone)
            let finishes = time(keys["finishesAt"], locale: locale, timeZone: timeZone)
            return isJapanese ? recoveryJa(org, started: started, finishes: finishes) : recoveryEn(org, started: started, finishes: finishes)
        case "root":
            // A count that is not a whole number counts one change.
            let waiting = (keys["waiting"] as? Int) ?? 1
            return isJapanese ? rootJa(org, waiting: waiting) : rootEn(org, waiting: waiting)
        default:
            return nil
        }
    }

    // MARK: The words

    private static func recoveryJa(_ org: String?, started: String?, finishes: String?) -> String {
        let head = org.map { "\($0) のルートキーの回復が始まりました" } ?? "組織のルートキーの回復が始まりました"
        return head + (started.map { "（\($0)）" } ?? "") + "。" + (finishes.map { "\($0) に有効になります。" } ?? "") + "机で確認してください。"
    }

    private static func recoveryEn(_ org: String?, started: String?, finishes: String?) -> String {
        let head = org.map { "A recovery of \($0) was started" } ?? "A recovery of an org was started"
        return head + (started.map { " at \($0)" } ?? "") + ". " + (finishes.map { "It takes effect at \($0). " } ?? "") + "Open the desk to see it."
    }

    private static func rootJa(_ org: String?, waiting: Int) -> String {
        (org.map { "\($0) の" } ?? "") + "変更" + (waiting > 1 ? " \(waiting) 件が" : "が") + "、ルートキーの署名を待っています。机で確認してください。"
    }

    private static func rootEn(_ org: String?, waiting: Int) -> String {
        let many = waiting > 1
        return (many ? "\(waiting) changes" : "A change") + (org.map { " to \($0)" } ?? "") + (many ? " wait" : " waits")
            + " for the root key’s signature. Open the desk to see " + (many ? "them" : "it") + "."
    }

    // MARK: Times

    /// A time as the viewer reads one (their language and zone), or `nil` when the payload's is not a time.
    private static func time(_ value: Any?, locale: Locale, timeZone: TimeZone) -> String? {
        guard let text = value as? String, let date = date(from: text) else { return nil }
        let formatter = DateFormatter()
        formatter.locale = locale
        formatter.timeZone = timeZone
        // The web's reading of a time (`Intl`, month short, day, hour and minute): `11月26日 18:30`, `Nov 26, 4:30 AM`.
        if locale.identifier.hasPrefix("ja") { formatter.setLocalizedDateFormatFromTemplate("MMMd jmm") } else { formatter.dateFormat = "MMM d, h:mm a" }
        return formatter.string(from: date)
    }

    private static func date(from text: String) -> Date? {
        let plain = ISO8601DateFormatter()
        plain.formatOptions = [.withInternetDateTime]
        if let date = plain.date(from: text) { return date }
        let fractional = ISO8601DateFormatter()
        fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return fractional.date(from: text)
    }
}
