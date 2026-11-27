import AgencyAPI
import Foundation

/// What a run of the app with no window does with the decisions a node has not taken yet: sends what is due, and says what is left. The
/// envelopes were signed when they were decided, so nothing here needs the device key or a screen.
@MainActor
public enum BackgroundSending {
    public struct Result: Equatable, Sendable {
        /// The items that still have a decision kept: the desk's counts leave them.
        public let decided: Set<String>
        /// When to try again, for a decision that waits for it (now, for one the run was cut short before it tried). `nil` when nothing
        /// waits, or when sending needs the owner first (the sign-in ended, or a decision stopped for good): a later try would find the same.
        public let retryAt: Date?
    }

    /// Sends every decision that is due from `storage`, once.
    public static func run(
        storage: any OutboxStorage, submitter: any DecisionSubmitter, now: @escaping @Sendable () -> Date = { Date() }
    ) async -> Result {
        let outbox = DecisionOutbox(storage: storage, submitter: submitter, now: now)
        await outbox.flush()
        return Result(decided: outbox.decidedItemIDs, retryAt: outbox.retryAt)
    }
}
