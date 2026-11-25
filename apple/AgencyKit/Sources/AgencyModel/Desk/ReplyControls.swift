import AgencyAPI
import Foundation

/// What the owner can press for one open request, given what they have picked and typed. The screen
/// only draws it, so the rules are tested here.
public struct ReplyControls: Equatable, Sendable {
    /// The options can be picked.
    public let canPick: Bool
    /// The main button: decide, or answer when the request has no options.
    public let canDecide: Bool
    /// The comment button: text and no decision. Only for a request that has options.
    public let showsComment: Bool
    public let canComment: Bool
    /// What the main button says: the verb, and with an option picked, that option.
    public let decideLabel: String

    public init(item: DeskItem, draft: DeskDraft, isSending: Bool, isSent: Bool) {
        let request = item.request
        let isLocked = !item.isMine || isSending || isSent
        let hasText = !draft.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        let hasOptions = !request.options.isEmpty
        let picked = draft.optionKey.flatMap { key in request.options.first { $0.key == key } }

        canPick = !isLocked
        canDecide = !isLocked && (hasOptions ? picked != nil : hasText)
        showsComment = hasOptions
        canComment = !isLocked && hasOptions && hasText
        let verb = Ja.Paper.verb(request.kind)
        decideLabel = picked.map { Ja.Paper.sendWith(verb, choice: $0.text) } ?? verb
    }
}
