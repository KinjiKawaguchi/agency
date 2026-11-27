import AgencyAPI
import Foundation

/// What a failed send of a kept decision comes to, and when it is tried again.
public enum SendFailure: Equatable, Sendable {
    /// The node answered 2xx but in words this app does not read: the node took it (nothing but a 2xx gets to be read).
    case taken
    /// Nothing the owner can mend: no network, a node that is busy or down, a runtime that did not answer in time.
    case retry(reason: String)
    /// A refusal that sending again will not change: not the account's to decide, a bad signature, expired, malformed.
    case final(reason: String)
    /// The Access sign-in has ended: nothing is sent until it runs again, and nothing is dropped.
    case signInEnded

    public static func of(_ error: Error) -> SendFailure {
        switch error {
        case NodeError.sessionExpired, NodeError.notSignedIn:
            return .signInEnded
        case NodeError.unreadable:
            return .taken
        case NodeError.http(let status, let message):
            let reason = message ?? "HTTP \(status)"
            return isTemporary(status: status) ? .retry(reason: reason) : .final(reason: reason)
        case is CancellationError:
            return .retry(reason: "")
        default:
            // A transport that did not get an answer (no network, a timeout, a lost connection): the same envelope goes again.
            return .retry(reason: error.localizedDescription)
        }
    }

    /// A status that another try may change: the request did not get to be judged (timeout, too many, the node or what is behind it
    /// busy or down).
    static func isTemporary(status: Int) -> Bool {
        status == 408 || status == 425 || status == 429 || (500...599).contains(status)
    }

    /// Seconds to wait after the `attempts`-th failed try: 5 s, 15 s, 1 min, 5 min, and then 5 min each time.
    public static func delay(afterAttempts attempts: Int) -> TimeInterval {
        let steps: [TimeInterval] = [5, 15, 60, 300]
        return steps[min(max(attempts, 1), steps.count) - 1]
    }
}
