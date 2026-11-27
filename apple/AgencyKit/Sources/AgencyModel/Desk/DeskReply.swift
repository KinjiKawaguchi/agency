import AgencyAPI
import AgencyProtocol
import Foundation

/// What the owner does with one request.
public enum DeskReply: Equatable, Sendable {
    /// Picks one of the request's options, with a note if they like. The request is decided.
    case choose(optionKey: String, note: String)
    /// Writes to the team without picking an option. The turn goes back to the team and nothing is
    /// decided. Only a request that has options can be commented on this way.
    case comment(String)
    /// Answers a request that has no options, in words.
    case answer(String)
}

public enum DeskReplyError: Error, Equatable, Sendable {
    /// Only the viewer's own requests can be decided.
    case notYours
    case unknownOption(String)
    /// A request with options is decided with one of them, or commented on.
    case optionsNeedAChoiceOrAComment
    case textRequired
    /// A request that has no options takes its answer as text.
    case noOptionsToChoose
    /// The node names a kind of envelope this app does not know how to make.
    case cannotAnswerHere
}

/// A reply, ready to be put into an envelope and sent.
public struct PreparedDecision: Equatable, Sendable {
    public static let maxNote = 200

    public let org: String
    public let team: String
    public let answerWith: AnswerWith
    /// What kind of request it answers, for the words that name the action.
    public let kind: Request.Kind
    /// The request's title, for the decision log (the request leaves the team's view once answered).
    /// At most 200 characters: the node refuses a longer note.
    public let title: String
    public let text: String
    public let choice: String?
    /// What the chosen option says, for the decision log; absent when it says nothing.
    public let choiceLabel: String?
    /// Text with no option picked, on a request that has options: the team is told and nothing is decided.
    public let isComment: Bool

    init(item: DeskItem, text: String, option: RequestOption?, isComment: Bool) {
        org = item.org
        team = item.team
        answerWith = item.request.answerWith
        kind = item.request.kind
        // Sent to the node for its decision log: what the runtime wrote, and not what the screen shows of it.
        title = Self.clipped(item.request.rawTitle)
        self.text = text
        choice = option?.key
        choiceLabel = option.map { Self.clipped($0.rawText) }.flatMap { $0.isEmpty ? nil : $0 }
        self.isComment = isComment
    }

    /// Trimmed, and when longer than the node keeps, cut so that the text and its ellipsis fit: 199
    /// scalars with their trailing space removed, then `…`. Characters are Unicode scalars, as the node
    /// counts them (Rust `chars`), not graphemes or UTF-16 units. The web client cuts the same way.
    static func clipped(_ note: String) -> String {
        let trimmed = note.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.unicodeScalars.count > maxNote else { return trimmed }
        var scalars = String.UnicodeScalarView()
        scalars.append(contentsOf: trimmed.unicodeScalars.prefix(maxNote - 1))
        while let last = scalars.last, last.properties.isWhitespace { scalars.removeLast() }
        return String(scalars) + "…"
    }

    /// The envelope this decision is signed as.
    public func envelope(id: String, account: String, at: String) -> DecisionEnvelope {
        DecisionEnvelope(
            id: id, kind: answerWith.kind, org: org, team: team, target: answerWith.target,
            text: text, choice: choice, account: account, at: at
        )
    }
}

extension DeskItem {
    /// Checks a reply against the request and prepares it. Nothing here asks the owner for anything or sends anything.
    public func prepare(_ reply: DeskReply) throws -> PreparedDecision {
        guard isMine else { throw DeskReplyError.notYours }
        guard isAnswerableHere else { throw DeskReplyError.cannotAnswerHere }
        let options = request.options
        switch reply {
        case .choose(let key, let note):
            guard !options.isEmpty else { throw DeskReplyError.noOptionsToChoose }
            guard let option = options.first(where: { $0.key == key }) else { throw DeskReplyError.unknownOption(key) }
            return PreparedDecision(item: self, text: Self.trimmed(note), option: option, isComment: false)
        case .comment(let text):
            guard !options.isEmpty else { throw DeskReplyError.noOptionsToChoose }
            let words = Self.trimmed(text)
            guard !words.isEmpty else { throw DeskReplyError.textRequired }
            return PreparedDecision(item: self, text: words, option: nil, isComment: true)
        case .answer(let text):
            guard options.isEmpty else { throw DeskReplyError.optionsNeedAChoiceOrAComment }
            let words = Self.trimmed(text)
            guard !words.isEmpty else { throw DeskReplyError.textRequired }
            return PreparedDecision(item: self, text: words, option: nil, isComment: false)
        }
    }

    private static func trimmed(_ text: String) -> String {
        text.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}

/// What the owner has picked or typed for one request and not sent yet. Kept per item until it is sent.
public struct DeskDraft: Codable, Equatable, Sendable {
    public var text: String
    public var optionKey: String?

    public init(text: String = "", optionKey: String? = nil) {
        self.text = text
        self.optionKey = optionKey
    }

    public var isEmpty: Bool { text.isEmpty && optionKey == nil }

    /// The reply this draft makes for `item` when the owner presses the main button: the picked option
    /// with the text as its note, or the text as the answer when the request has no options. A draft with
    /// text and no pick on a request with options is not a decision: it is sent through the comment button.
    public func decision(for item: DeskItem) -> DeskReply? {
        if item.request.options.isEmpty { return .answer(text) }
        return optionKey.map { .choose(optionKey: $0, note: text) }
    }
}
