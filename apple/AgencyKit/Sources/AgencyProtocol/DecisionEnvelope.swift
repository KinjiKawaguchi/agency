import Foundation

public enum EnvelopeError: Error, Equatable, Sendable {
    case malformedID(field: String)
    case malformedTarget
    case malformedChoice
    case malformedTime
    case textTooLong
    /// Neither a choice nor an answer text.
    case empty
}

/// A decision envelope: one answer to a request, or one decision on a board item. A device key
/// signs it (`signedText(kind: .envelope, ...)`) and the team's node applies it.
///
/// A comment is an answer with text and no choice: the team gets the message and nothing is decided.
public struct DecisionEnvelope: Equatable, Sendable {
    public enum Kind: String, Sendable {
        /// Answers a request (`target` is the request id).
        case answer
        /// Decides a board item that waits for a decision (`target` is `repo#N`).
        case judge
    }

    public static let maxTextLength = 4000
    public static let maxChoiceLength = 20

    public let id: String
    public let kind: Kind
    public let org: String
    public let team: String
    public let target: String
    public let text: String
    public let choice: String?
    public let account: String
    public let at: String

    public init(
        id: String, kind: Kind, org: String, team: String, target: String,
        text: String, choice: String?, account: String, at: String
    ) {
        self.id = id
        self.kind = kind
        self.org = org
        self.team = team
        self.target = target
        self.text = text
        self.choice = choice
        self.account = account
        self.at = at
    }

    /// The body that is signed and sent. `text` is always there, empty when only a choice is made.
    public var json: JSONValue {
        var payload: [JSONValue.Member] = [.init("text", .string(text))]
        if let choice { payload.append(.init("choice", .string(choice))) }
        return .object([
            .init("id", .string(id)), .init("kind", .string(kind.rawValue)),
            .init("org", .string(org)), .init("team", .string(team)),
            .init("target", .string(target)), .init("payload", .object(payload)),
            .init("account", .string(account)), .init("at", .string(at)),
        ])
    }

    /// The shape the node requires, checked before the owner is asked for Face ID. The node checks it
    /// again, and also the signature, membership and time.
    public func validate() throws {
        guard Self.isEnvelopeID(id) else { throw EnvelopeError.malformedID(field: "id") }
        for (field, value) in [("org", org), ("team", team), ("account", account)] where !Self.isID(value) {
            throw EnvelopeError.malformedID(field: field)
        }
        guard Self.isTarget(target) else { throw EnvelopeError.malformedTarget }
        guard text.unicodeScalars.count <= Self.maxTextLength else { throw EnvelopeError.textTooLong }
        if let choice, choice.isEmpty || choice.unicodeScalars.count > Self.maxChoiceLength {
            throw EnvelopeError.malformedChoice
        }
        guard Timestamp.date(from: at) != nil else { throw EnvelopeError.malformedTime }
        let hasText = !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        guard hasText || choice != nil else { throw EnvelopeError.empty }
    }

    /// `[a-z0-9][a-z0-9-]{0,39}`
    static func isID(_ value: String) -> Bool {
        let bytes = Array(value.utf8)
        func isHead(_ byte: UInt8) -> Bool { (97...122).contains(byte) || (48...57).contains(byte) }
        return (1...40).contains(bytes.count) && isHead(bytes[0]) && bytes.allSatisfy { isHead($0) || $0 == 45 }
    }

    /// 16 random bytes as 32 lowercase hex characters.
    static func isEnvelopeID(_ value: String) -> Bool {
        value.utf8.count == 32 && value.utf8.allSatisfy { (48...57).contains($0) || (97...102).contains($0) }
    }

    /// 1 to 120 characters of letters, digits and `._#-`.
    static func isTarget(_ value: String) -> Bool {
        let bytes = Array(value.utf8)
        return (1...120).contains(bytes.count) && bytes.allSatisfy {
            (48...57).contains($0) || (65...90).contains($0) || (97...122).contains($0) || [46, 95, 35, 45].contains($0)
        }
    }

    /// A new envelope id from 16 bytes of randomness.
    public static func makeID(randomBytes: [UInt8]) -> String? {
        randomBytes.count == 16 ? randomBytes.map { String(format: "%02x", $0) }.joined() : nil
    }
}
