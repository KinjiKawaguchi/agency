import AgencyAPI
import Foundation

/// One reading of a KPI around the day an epic shipped, in plain words. Every state says what it says and no more: a
/// number is given only when the node gave one (`ready`), the others say what is missing, and none says that the work
/// moved the number.
public struct ReadingLine: Equatable, Identifiable, Sendable {
    public let id: String
    /// The team's own label.
    public let team: String
    /// The epic's title, or its reference when its team's view no longer lists it.
    public let epic: String
    public let kpi: String
    /// What the reading says, whatever its state.
    public let headline: String
    /// With `ready`: how the number moved from before the work shipped to after.
    public let movement: Movement?
    /// With `ready`: the line that says the movement is not a cause.
    public let note: String?
    /// With `ready`: the other epics whose effects cannot be told apart from this one's.
    public let overlap: String?

    /// - Parameter today: the day the node made the readings, which a day in the same year is written without its year.
    public init(reading: AimReading, team: String, kpi: Kpi?, today: String? = nil) {
        id = "\(reading.team)/\(reading.epic)/\(reading.kpi)"
        self.team = team
        epic = reading.epicTitle ?? reading.epic
        self.kpi = kpi?.label ?? reading.kpi
        let said = Self.say(reading, kpi: kpi, today: today)
        headline = said.headline
        movement = said.movement
        note = said.note
        overlap = said.overlap
    }

    private struct Said {
        var headline: String
        var movement: Movement?
        var note: String?
        var overlap: String?

        init(_ headline: String) { self.headline = headline }
    }

    private static func say(_ reading: AimReading, kpi: Kpi?, today: String?) -> Said {
        switch reading.state {
        case .ready: ready(reading, kpi: kpi, today: today)
        case .notShipped: Said(Ja.Outcomes.notShipped)
        case .shipDateUnknown: Said(Ja.Outcomes.shipDateUnknown)
        case .noFollowUp: Said(Ja.Outcomes.noFollowUp(windowDays: AimReading.windowDays))
        case .noBaseline: Said(reading.shippedOn.map { Ja.Outcomes.noBaseline(day: OutcomeFormat.day($0, relativeTo: today), windowDays: AimReading.windowDays) } ?? Ja.Outcomes.unreadable)
        case .tooEarly: Said(reading.readableFrom.map { Ja.Outcomes.tooEarly(readableFrom: OutcomeFormat.day($0, relativeTo: today), settleDays: AimReading.settleDays) } ?? Ja.Outcomes.unreadable)
        case .tooFew:
            if let points = reading.points, let needs = reading.needs { Said(Ja.Outcomes.tooFew(points: points, needs: needs)) } else { Said(Ja.Outcomes.unreadable) }
        case .unknown: Said(Ja.Outcomes.unreadable)
        }
    }

    /// A `ready` reading has both numbers; one that does not is not one this app can draw, and claims nothing.
    private static func ready(_ reading: AimReading, kpi: Kpi?, today: String?) -> Said {
        guard let before = reading.before, let after = reading.after else { return Said(Ja.Outcomes.unreadable) }
        let unit = kpi?.unit ?? .unknown("")
        let moved = Movement(from: before.value, to: after.value, unit: unit, goodWhen: kpi?.goodWhen ?? .unknown(""))
        var said = Said(Ja.Outcomes.ready(
            before: OutcomeFormat.value(before.value, unit: unit), beforeDay: OutcomeFormat.day(before.date, relativeTo: today),
            after: OutcomeFormat.value(after.value, unit: unit), afterDay: OutcomeFormat.day(after.date, relativeTo: today)
        ))
        said.movement = moved
        said.note = Ja.Outcomes.correlation
        said.overlap = reading.overlapping.isEmpty ? nil : Ja.Outcomes.overlap(reading.overlapping)
        return said
    }
}
