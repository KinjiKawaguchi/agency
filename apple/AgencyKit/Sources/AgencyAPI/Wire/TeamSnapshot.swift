import Foundation

/// `GET /api/orgs/<org>/teams/<team>/view`: a team's view, who the viewer is in it, and how fresh it is.
public struct TeamSnapshot: Decodable, Equatable, Sendable {
    public let view: TeamView
    /// The viewer's name in the team's runtime.
    public let viewer: String?
    /// The ids of the requests that are the viewer's to decide. Clients use this and never work it out from `to`.
    public let mine: [String]
    /// The ids of the people whose `waitingOn` is the viewer.
    public let waitingOnViewer: [String]
    public let source: Source

    public enum Source: Decodable, Equatable, Sendable {
        case local
        /// A copy from the team's node; `error` says why the last read failed.
        case replica(fetchedAt: Instant?, triedAt: Instant, error: String?)
        /// A source a newer node names: its view is read and shown as the node gave it, with no word on its age.
        case unknown(String)

        private enum Keys: String, CodingKey { case kind, fetchedAt, triedAt, error }

        public init(from decoder: any Decoder) throws {
            let container = try decoder.container(keyedBy: Keys.self)
            switch try container.decode(String.self, forKey: .kind) {
            case "local":
                self = .local
            case "replica":
                self = .replica(
                    fetchedAt: try container.decodeIfPresent(Instant.self, forKey: .fetchedAt),
                    triedAt: try container.decode(Instant.self, forKey: .triedAt),
                    error: try container.decodeIfPresent(String.self, forKey: .error)
                )
            case let other:
                self = .unknown(other)
            }
        }
    }
}
