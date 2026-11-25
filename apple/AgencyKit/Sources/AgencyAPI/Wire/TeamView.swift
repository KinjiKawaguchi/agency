import AgencyProtocol
import Foundation

/// What a team runtime reports about its team.
public struct TeamView: Decodable, Equatable, Sendable {
    public let org: String
    public let team: String
    public let label: String
    public let generatedAt: Instant
    public let people: [PersonStatus]
    public let requests: [Request]
    public let work: [WorkItem]
    public let epics: [EpicForecast]
}

public struct PersonStatus: Decodable, Equatable, Sendable {
    public enum Kind: String, Decodable, Sendable { case agent, human }
    public enum Role: String, Decodable, Sendable { case lead, member }
    public enum State: String, Decodable, Sendable { case working, idle, away, unknown }

    /// The team runtime's own name for them.
    public let id: String
    public let label: String
    public let kind: Kind
    public let role: Role
    public let state: State
    public let since: Instant?
    public let waitingOn: WaitingOn?
    public let status: String
    public let holding: [String]
}

/// Who has to act before this person can continue.
public enum WaitingOn: Decodable, Equatable, Sendable {
    case person(String)
    case lead

    private enum Keys: String, CodingKey { case kind, person }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "person": self = .person(try container.decode(String.self, forKey: .person))
        case "lead": self = .lead
        case let other:
            throw DecodingError.dataCorruptedError(forKey: .kind, in: container, debugDescription: "unknown waitingOn kind \(other)")
        }
    }
}

/// Something the team asks a person to decide or answer.
public struct Request: Decodable, Equatable, Sendable {
    public enum Kind: String, Decodable, Sendable { case question, proposal, pendingDecision }

    public let id: String
    public let kind: Kind
    public let title: String
    /// Plain text: the background needed to decide.
    public let context: String
    /// Markdown body of a proposal.
    public let document: String?
    public let from: String
    /// The addressee by the team runtime's name; absent means the team's owner.
    public let to: String?
    public let createdAt: Instant
    public let issue: String?
    public let blocks: [String]
    public let options: [RequestOption]
    /// Earlier messages, oldest first.
    public let thread: [ThreadMessage]?
    /// Exactly the `kind` and `target` of the envelope that answers it.
    public let answerWith: AnswerWith
}

public struct RequestOption: Decodable, Equatable, Sendable {
    public let key: String
    public let text: String
    public let isRecommended: Bool
    public let detail: String?
}

public struct ThreadMessage: Decodable, Equatable, Sendable {
    public let from: String
    public let text: String
    public let choice: String?
    public let at: Instant
}

public struct AnswerWith: Decodable, Equatable, Sendable {
    public let kind: DecisionEnvelope.Kind
    public let target: String
}

public struct WorkItem: Decodable, Equatable, Sendable {
    public enum Status: String, Decodable, Sendable { case backlog, next, inProgress, release, done }

    /// `repo#N`
    public let ref: String
    public let title: String
    public let status: Status
    public let assignee: String?
    public let blocked: Blocked?
    public let epic: String?
    public let points: Int?
    public let eta: Eta?
}

public enum BlockedReason: String, Decodable, Sendable {
    case needsDecision, external, waitingOnPrevious, timeOrEquipment
}

public struct Blocked: Decodable, Equatable, Sendable {
    public let reason: BlockedReason
    /// A calendar date, `YYYY-MM-DD`.
    public let since: String
}

public struct Eta: Decodable, Equatable, Sendable {
    public let p50: Instant
    public let p80: Instant
    public let basis: String
    public let isOverBudget: Bool
}

public struct EpicForecast: Decodable, Equatable, Sendable {
    public let ref: String
    public let title: String
    public let total: Int
    public let done: Int
    public let doing: Int
    public let remainingPoints: Int
    public let blocked: [BlockedCount]
    public let forecast: Forecast?
    public let basis: String
}

public struct BlockedCount: Decodable, Equatable, Sendable {
    public let reason: BlockedReason
    public let count: Int
}

public struct Forecast: Decodable, Equatable, Sendable {
    public let days: Int
    public let at: Instant
}
