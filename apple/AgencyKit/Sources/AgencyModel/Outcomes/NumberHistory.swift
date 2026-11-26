import AgencyAPI
import Foundation

/// One entry of a KPI's history, as the page lists it: the day, the number by the KPI's unit, where it came from, when it was
/// entered, its note, and whether a later entry for the same day has replaced it (the first stays in the history, and the later
/// one wins in every reading).
public struct NumberHistoryEntry: Equatable, Identifiable, Sendable {
    public let id: String
    public let day: String
    public let value: String
    public let source: String
    public let enteredOn: String
    public let note: String?
    public let isReplaced: Bool

    public init(day: String, value: String, source: String, enteredOn: String, note: String?, isReplaced: Bool, id: String) {
        self.id = id
        self.day = day
        self.value = value
        self.source = source
        self.enteredOn = enteredOn
        self.note = note
        self.isReplaced = isReplaced
    }

    /// The entries of a history (newest first, as the node gives it) as the page lists them.
    public static func list(_ entries: [KpiObservation], unit: Kpi.Unit, until: String, timeZone: TimeZone = .current) -> [NumberHistoryEntry] {
        let replaced = replacedIn(entries)
        return entries.enumerated().map { index, entry in
            NumberHistoryEntry(
                day: OutcomeFormat.day(entry.date, relativeTo: until),
                value: OutcomeFormat.value(entry.value, unit: unit),
                source: entry.source == .manual ? Ja.Outcomes.History.manual : Ja.Outcomes.History.connector(name(of: entry.source)),
                enteredOn: Ja.Outcomes.History.enteredOn(JaFormat.dateTime(entry.enteredAt.date, in: timeZone)),
                note: entry.note,
                isReplaced: replaced[index],
                id: "\(index)/\(entry.enteredAt.text)/\(entry.date)"
            )
        }
    }

    private static func name(of source: KpiObservation.Source) -> String {
        if case .connector(let name) = source { name } else { "" }
    }

    /// For each entry, whether a later one for the same KPI and day replaced it. Times are compared as instants: RFC 3339 texts
    /// do not sort as text once a fraction of a second or a zone other than `Z` comes in.
    public static func replacedIn(_ entries: [KpiObservation]) -> [Bool] {
        var latest: [String: Date] = [:]
        for entry in entries {
            let key = "\(entry.kpi)/\(entry.date)"
            if let known = latest[key], known >= entry.enteredAt.date { continue }
            latest[key] = entry.enteredAt.date
        }
        return entries.map { entry in
            guard let newest = latest["\(entry.kpi)/\(entry.date)"] else { return false }
            return entry.enteredAt.date < newest
        }
    }
}
