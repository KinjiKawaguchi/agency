import AgencyProtocol
import Foundation

/// `GET /api/decisions`: the account's recent decisions, newest first.
public struct DecisionLog: Decodable, Equatable, Sendable {
    public let entries: [DecisionEntry]
}

public struct DecisionEntry: Decodable, Equatable, Sendable {
    /// The decision envelope's id.
    public let id: String
    public let org: String
    public let team: String
    public let kind: DecisionEnvelope.Kind
    /// The request id or work item ref it answered.
    public let target: String
    public let title: String?
    public let text: String
    public let choice: String?
    public let choiceLabel: String?
    public let decidedAt: Instant
    public let state: DecisionState

    /// A comment: text and no choice. Nothing was decided.
    public var isComment: Bool { choice == nil }
}

/// Where a decision went.
public enum DecisionState: Decodable, Equatable, Sendable {
    case applied
    case delivered
    case queued(attempts: Int, nextAt: Instant, lastError: String?)
    case refused(message: String)

    private enum Keys: String, CodingKey { case kind, attempts, nextAt, lastError, message }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "applied": self = .applied
        case "delivered": self = .delivered
        case "queued":
            self = .queued(
                attempts: try container.decode(Int.self, forKey: .attempts),
                nextAt: try container.decode(Instant.self, forKey: .nextAt),
                lastError: try container.decodeIfPresent(String.self, forKey: .lastError)
            )
        case "refused": self = .refused(message: try container.decode(String.self, forKey: .message))
        case let other:
            throw DecodingError.dataCorruptedError(forKey: .kind, in: container, debugDescription: "unknown decision state \(other)")
        }
    }
}

/// The answer to `POST /api/orgs/<org>/teams/<team>/envelopes`.
public struct DecisionResult: Decodable, Equatable, Sendable {
    public enum Outcome: String, Decodable, Sendable { case applied, delivered, queued }

    public let outcome: Outcome
    /// Why a decision was queued.
    public let message: String?
}
