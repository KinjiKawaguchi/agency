import AgencyAPI
import AgencyProtocol
import Foundation

/// A decision that is signed and kept on the device until the node has taken it (agency#447). Everything the send needs is in it: the
/// signed envelope as it will be sent, again and again if need be (the same id, so the node keeps it once), and the words the sending
/// tray shows. It survives the app being closed or killed.
public struct PendingDecision: Codable, Equatable, Identifiable, Sendable {
    /// Why the last send did not take.
    public struct Failure: Codable, Equatable, Sendable {
        /// What the node said, or what went wrong on the way, as it came: never made up.
        public let reason: String
        /// A refusal that will not change by sending again: only the owner can act on it (discard it and decide again).
        public let isFinal: Bool

        public init(reason: String, isFinal: Bool) {
            self.reason = reason
            self.isFinal = isFinal
        }
    }

    /// The envelope's id: what makes a resend the same decision.
    public let id: String
    /// The desk item it decides (`DeskItem.id`): it does not come back on the pile while this entry exists.
    public let itemID: String
    public let org: String
    public let team: String
    public let kind: String
    public let target: String
    public let text: String
    public let choice: String?
    public let account: String
    public let at: String
    public let signer: String
    public let signature: String
    /// The request's title and the chosen option's label, for the node's decision log (never signed).
    public let title: String?
    public let choiceLabel: String?
    /// A comment, not a decision: the tray's words say so.
    public let isComment: Bool
    public let createdAt: Date
    public private(set) var attempts: Int
    /// Not before: after a failure that sending again may mend. `nil` is now.
    public private(set) var nextAttemptAt: Date?
    public private(set) var failure: Failure?

    public init(itemID: String, signed: SignedDecision, isComment: Bool, createdAt: Date) {
        id = signed.envelope.id
        self.itemID = itemID
        org = signed.envelope.org
        team = signed.envelope.team
        kind = signed.envelope.kind.rawValue
        target = signed.envelope.target
        text = signed.envelope.text
        choice = signed.envelope.choice
        account = signed.envelope.account
        at = signed.envelope.at
        signer = signed.signer
        signature = signed.signature
        title = signed.title
        choiceLabel = signed.choiceLabel
        self.isComment = isComment
        self.createdAt = createdAt
        attempts = 0
        nextAttemptAt = nil
        failure = nil
    }

    /// The decision as it is sent: the very envelope that was signed.
    public var signed: SignedDecision {
        SignedDecision(
            envelope: DecisionEnvelope(
                id: id, kind: DecisionEnvelope.Kind(rawValue: kind) ?? .unknown, org: org, team: team, target: target,
                text: text, choice: choice, account: account, at: at
            ),
            signer: signer, signature: signature, title: title, choiceLabel: choiceLabel
        )
    }

    /// Whether it waits for the owner to act on a refusal.
    public var isFinallyRefused: Bool { failure?.isFinal == true }

    /// Whether it is due for a send at `now`.
    public func isDue(at now: Date) -> Bool {
        !isFinallyRefused && (nextAttemptAt ?? .distantPast) <= now
    }

    mutating func failed(_ failure: Failure, retryAt: Date?) {
        attempts += 1
        self.failure = failure
        nextAttemptAt = failure.isFinal ? nil : retryAt
    }

    /// For tests and renderings: an entry as it is after a failure.
    public mutating func failedForTest(_ failure: Failure) {
        failed(failure, retryAt: nil)
    }

    /// "Send again": whatever stopped it is forgotten, and it is due now.
    mutating func readyToSendAgain() {
        failure = nil
        nextAttemptAt = nil
    }
}
