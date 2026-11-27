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
    @ShownIfPresent public private(set) var title: String?
    @ShownText public private(set) var text: String
    public let choice: String?
    @ShownIfPresent public private(set) var choiceLabel: String?
    /// The seat or external party the decision answered for, when it did: the node worked it out when it applied the decision, and
    /// no client says it. Present only for a team on the node that applied it.
    public let relayedFor: RelayedFor?
    public let decidedAt: Instant
    public let state: DecisionState

    /// A comment: text and no choice. Nothing was decided.
    public var isComment: Bool { choice == nil }

    /// Whom the decision answered for, when the node says and the label is not empty or only spaces.
    public var answeredFor: String? {
        let trimmed = relayedFor?.label.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.flatMap { $0.isEmpty ? nil : $0 }
    }
}

/// The seat or external party a decision answered for: its id, and its label as the org's definition had it when the decision was applied.
public struct RelayedFor: Decodable, Equatable, Sendable {
    public let id: String
    @Shown public private(set) var label: String

    public init(id: String, label: String) {
        self.id = id
        self.label = label
    }
}

/// Where a decision went.
public enum DecisionState: Decodable, Equatable, Sendable {
    case applied
    case delivered
    case queued(attempts: Int, nextAt: Instant, lastError: String?)
    case refused(message: String)
    /// A state a newer node names: read, never taken for one of the others (a success among them).
    case unknown(String)

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
        case let other: self = .unknown(other)
        }
    }
}

/// The answer to `POST /api/orgs/<org>/teams/<team>/envelopes`.
public struct DecisionResult: Decodable, Equatable, Sendable {
    public enum Outcome: String, OpenWireEnum { case applied, delivered, queued, unknown }

    public let outcome: Outcome
    /// Why a decision was queued.
    @ShownTextIfPresent public private(set) var message: String?
}
