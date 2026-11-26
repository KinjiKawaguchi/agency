import AgencyProtocol
import Foundation

/// `GET /api/orgs/<org>/outcomes?days=<1 to 90>`: the numbers the owner keeps for what the org measures.
///
/// Business data: the node answers only the owner (403 to anyone else), keeps the numbers on its own machine, and a
/// client keeps them in memory only. A value is in the unit of its KPI and is never converted (money stays in its own
/// currency). What a newer node may add is read as "unknown" rather than refused, so the screen keeps working: an
/// unknown unit shows a plain number, an unknown direction colours nothing.
public struct Outcomes: Decodable, Equatable, Sendable {
    public let org: String
    public let days: Int
    /// The period's first and last calendar day in the node's time zone, as in `Evaluation`.
    public let from: String
    public let until: String
    public let generatedAt: Instant
    /// One for each KPI the org's definition lists, in its order.
    public let series: [KpiSeries]
}

public struct KpiSeries: Decodable, Equatable, Sendable {
    public let kpi: Kpi
    /// The numbers of the period, oldest first, one for each day.
    public let points: [KpiObservation]
    /// The last number before the period: the baseline a movement is read from.
    public let before: KpiObservation?
}

/// A measure the org states. It holds no number.
public struct Kpi: Decodable, Equatable, Sendable {
    public enum Kind: Equatable, Sendable {
        case business, product
        case unknown(String)
    }

    public enum Unit: Equatable, Sendable {
        /// In its own currency (an ISO 4217 code), never converted.
        case money(currency: String)
        case percent
        case count
        case ratio
        /// A duration.
        case seconds
        case unknown(String)
    }

    public enum GoodWhen: Equatable, Sendable {
        case higher, lower
        case unknown(String)
    }

    public let id: String
    @Shown public private(set) var label: String
    public let kind: Kind
    public let unit: Unit
    public let goodWhen: GoodWhen

    private enum Keys: String, CodingKey { case id, label, kind, unit, currency, goodWhen }

    public init(from decoder: any Decoder) throws {
        let values = try decoder.container(keyedBy: Keys.self)
        id = try values.decode(String.self, forKey: .id)
        label = try values.decode(String.self, forKey: .label)
        kind = switch try values.decode(String.self, forKey: .kind) {
        case "business": .business
        case "product": .product
        case let other: .unknown(other)
        }
        unit = switch try values.decode(String.self, forKey: .unit) {
        case "money": .money(currency: try values.decode(String.self, forKey: .currency))
        case "percent": .percent
        case "count": .count
        case "ratio": .ratio
        case "seconds": .seconds
        case let other: .unknown(other)
        }
        goodWhen = switch try values.decode(String.self, forKey: .goodWhen) {
        case "higher": .higher
        case "lower": .lower
        case let other: .unknown(other)
        }
    }
}

/// One number for one KPI on one day (the protocol's `Observation`; named apart from the system's `Observation` module, which `@Observable` needs).
public struct KpiObservation: Decodable, Equatable, Sendable {
    public enum Source: Equatable, Sendable {
        case manual
        case connector(name: String)
    }

    public let kpi: String
    /// The calendar day the number is as of, in the node's time zone: `YYYY-MM-DD`.
    public let date: String
    public let value: Double
    public let source: Source
    public let enteredAt: Instant
    public let enteredBy: String
    public let note: String?

    private enum Keys: String, CodingKey { case kpi, date, value, source, enteredAt, enteredBy, note }
    private enum SourceKeys: String, CodingKey { case kind, name }

    public init(from decoder: any Decoder) throws {
        let values = try decoder.container(keyedBy: Keys.self)
        kpi = try values.decode(String.self, forKey: .kpi)
        date = try values.decode(String.self, forKey: .date)
        value = try values.decode(Double.self, forKey: .value)
        enteredAt = try values.decode(Instant.self, forKey: .enteredAt)
        enteredBy = try values.decode(String.self, forKey: .enteredBy)
        note = try values.decodeIfPresent(String.self, forKey: .note)
        let origin = try values.nestedContainer(keyedBy: SourceKeys.self, forKey: .source)
        source = switch try origin.decode(String.self, forKey: .kind) {
        case "connector": .connector(name: try origin.decode(String.self, forKey: .name))
        default: .manual
        }
    }
}

/// `GET /api/orgs/<org>/outcomes/readings`: how each KPI an aim names stood around the day its epic shipped.
///
/// A reading is arithmetic on the owner's own numbers around a day. Its basis is always a correlation: it says the
/// number moved after the work shipped, never that the work moved it.
public struct Readings: Decodable, Equatable, Sendable {
    public let org: String
    public let generatedAt: Instant
    public let readings: [AimReading]
}

public struct AimReading: Decodable, Equatable, Sendable {
    /// The days after the ship day before a number counts as after it, and how far from it a number is read against it:
    /// the node's constants (`SETTLE_DAYS` and `WINDOW_DAYS` in `agency-proto`), which a test keeps equal to these.
    public static let settleDays = 14
    public static let windowDays = 90

    public enum State: Equatable, Sendable {
        case notShipped
        case shipDateUnknown
        case noBaseline
        case tooEarly
        case noFollowUp
        case tooFew
        case ready
        /// A state a newer node has: nothing is claimed of it.
        case unknown(String)
    }

    public let team: String
    /// The epic's item reference, as the aim names it.
    public let epic: String
    public let epicTitle: String?
    public let kpi: String
    public let shippedOn: String?
    public let state: State
    /// With `tooFew`: how many numbers there are, and how many are needed.
    public let points: Int?
    public let needs: Int?
    /// With `tooEarly`: the first day a number would count as after the ship day.
    public let readableFrom: String?
    /// With `ready`: the last number on or before the ship day, the first at least 14 days after it, and the change.
    public let before: KpiObservation?
    public let after: KpiObservation?
    public let change: Double?
    public let changeRatio: Double?
    /// With `ready`: the other epics that aim at the same KPI and shipped in between; their effects cannot be told apart.
    public let overlapping: [String]

    private enum Keys: String, CodingKey {
        case team, epic, epicTitle, kpi, shippedOn, state, points, needs, readableFrom, before, after, change, changeRatio, overlapping
    }

    public init(from decoder: any Decoder) throws {
        let values = try decoder.container(keyedBy: Keys.self)
        team = try values.decode(String.self, forKey: .team)
        epic = try values.decode(String.self, forKey: .epic)
        epicTitle = try values.decodeIfPresent(String.self, forKey: .epicTitle)
        kpi = try values.decode(String.self, forKey: .kpi)
        shippedOn = try values.decodeIfPresent(String.self, forKey: .shippedOn)
        state = switch try values.decode(String.self, forKey: .state) {
        case "notShipped": .notShipped
        case "shipDateUnknown": .shipDateUnknown
        case "noBaseline": .noBaseline
        case "tooEarly": .tooEarly
        case "noFollowUp": .noFollowUp
        case "tooFew": .tooFew
        case "ready": .ready
        case let other: .unknown(other)
        }
        points = try values.decodeIfPresent(Int.self, forKey: .points)
        needs = try values.decodeIfPresent(Int.self, forKey: .needs)
        readableFrom = try values.decodeIfPresent(String.self, forKey: .readableFrom)
        before = try values.decodeIfPresent(KpiObservation.self, forKey: .before)
        after = try values.decodeIfPresent(KpiObservation.self, forKey: .after)
        change = try values.decodeIfPresent(Double.self, forKey: .change)
        changeRatio = try values.decodeIfPresent(Double.self, forKey: .changeRatio)
        // The list is left out when there is none; that is an answer, not a missing value.
        overlapping = try values.decodeIfPresent([String].self, forKey: .overlapping) ?? []
    }
}

/// What the owner enters: one number of a KPI for one day (`ObservationInput`). The node sets who entered it and when, so there is
/// no field for either.
public struct ObservationInput: Encodable, Equatable, Sendable {
    public let kpi: String
    /// The calendar day the number is as of, `YYYY-MM-DD`.
    public let date: String
    public let value: Double
    /// At most 200 characters, with no control character; left out when there is none.
    public let note: String?

    public init(kpi: String, date: String, value: Double, note: String? = nil) {
        self.kpi = kpi
        self.date = date
        self.value = value
        self.note = note
    }
}
