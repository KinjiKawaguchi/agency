import AgencyAPI
import Foundation

/// How a read that is not ready yet is asked again. A node answers 503 while a team's view is not ready
/// ("the board has not been read yet; a refresh was started", or the team's runtime did not answer in
/// time), and a slow node times out: none of that is a failure to show, so the read waits and asks again,
/// and an error is shown only when it keeps failing. The rule is the web client's (agency#170).
public struct RetryPolicy: Sendable {
    /// The waits between one attempt and the next; the read is attempted once more than there are waits.
    public let delays: [TimeInterval]
    private let wait: @Sendable (TimeInterval) async throws -> Void

    /// 1, 2, 4, 8 and 15 seconds: five retries, then it gives up for now.
    public static let standard = RetryPolicy(delays: [1, 2, 4, 8, 15])
    /// One attempt, for a read that is asked again by the person.
    public static let never = RetryPolicy(delays: [])

    /// - Parameter wait: how a wait is spent; a test hands in one that does not sleep.
    public init(delays: [TimeInterval], wait: @escaping @Sendable (TimeInterval) async throws -> Void = { try await Task.sleep(for: .seconds($0)) }) {
        self.delays = delays
        self.wait = wait
    }

    /// The statuses that say "not now" rather than "no": 408, 429, 502, 503 and 504. Every other 4xx is final.
    private static let retryableStatuses: Set<Int> = [408, 429, 502, 503, 504]

    /// An answer that says "not now" rather than "no", or no answer at all (offline, timed out). The expired
    /// sign-in, a refusal and a cancelled read are not retried.
    public static func isRetryable(_ error: Error) -> Bool {
        if case NodeError.http(let status, _) = error { return retryableStatuses.contains(status) }
        if let failure = error as? URLError { return failure.code != .cancelled }
        return false
    }

    /// Runs `operation`, and again after each wait while the answer is one that is retryable.
    /// Stops at once when the task is cancelled, and gives the last error when the waits are used up.
    public func run<Value: Sendable>(_ operation: @Sendable () async throws -> Value) async throws -> Value {
        var remaining = delays[...]
        while true {
            do { return try await operation() }
            catch where Self.isRetryable(error) {
                guard let delay = remaining.popFirst() else { throw error }
                try await wait(delay)
            }
        }
    }
}
