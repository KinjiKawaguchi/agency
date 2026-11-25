import Foundation

/// `GET /api/session`: the token that writes need, and the node's clock.
public struct Session: Decodable, Equatable, Sendable {
    public let writeToken: String
    /// The node's time, which `at` of a signed envelope is measured against.
    public let now: Instant
}

/// What the node says when a call fails: `{ "error": <text> }`.
struct ErrorBody: Decodable {
    let error: String
}
