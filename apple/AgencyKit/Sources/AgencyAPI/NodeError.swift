import Foundation

public enum NodeError: Error, Equatable, Sendable {
    /// There is no Access token yet, or the stored one has expired.
    case notSignedIn
    /// Access answered with a redirect to its login: the session is over and the sign-in has to run again.
    case sessionExpired
    /// The node answered with an error status. `message` is the node's own text when it sent one.
    case http(status: Int, message: String?)
    /// The answer is not what the client API documents.
    case unreadable(String)
}
