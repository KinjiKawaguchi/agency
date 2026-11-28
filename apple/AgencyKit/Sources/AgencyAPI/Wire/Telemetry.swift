import AgencyProtocol
import Foundation

/// What a client of the owner observed of its own calls (`POST /api/telemetry`, `docs/protocol.md`, "Metrics"): at most 200 samples and 16 KiB.
public struct TelemetryReport: Equatable, Sendable {
    public static let maxSamples = 200
    public static let maxBytes = 16 * 1024
    /// What the node calls the Apple apps' reports; the version and the platform are in `x-agency-client`.
    public static let clientName = "ios"

    public let samples: [CallSample]

    public init(samples: [CallSample]) {
        self.samples = samples
    }

    /// The request body: canonical JSON, the same bytes as the node reads.
    var body: Data {
        get throws { try Data(CanonicalJSON.encode(json).utf8) }
    }

    var json: JSONValue {
        .object([.init("client", .string(Self.clientName)), .init("samples", .array(samples.map(Self.json)))])
    }

    static func json(_ sample: CallSample) -> JSONValue {
        .object([.init("route", .string(sample.route)), .init("status", .string(sample.status)), .init("ms", .integer(sample.ms))])
    }

    /// The largest run of `pending`, from its first, that fits one report: no more samples than the node takes and no more bytes.
    public static func fitting(_ pending: [CallSample]) -> TelemetryReport {
        var taken: [CallSample] = []
        var size = (try? TelemetryReport(samples: []).body.count) ?? 0
        for sample in pending.prefix(maxSamples) {
            let own = ((try? Data(CanonicalJSON.encode(json(sample)).utf8).count) ?? Int.max) + 1
            guard size + own <= maxBytes else { break }
            size += own
            taken.append(sample)
        }
        return TelemetryReport(samples: taken)
    }
}

/// The answer to a report.
public struct TelemetryAnswer: Decodable, Equatable, Sendable {
    /// How many samples became part of a series, and how many the node dropped (a route it does not list, a status it does not read).
    public let accepted: Int
    public let dropped: Int

    public init(accepted: Int, dropped: Int) {
        self.accepted = accepted
        self.dropped = dropped
    }
}
