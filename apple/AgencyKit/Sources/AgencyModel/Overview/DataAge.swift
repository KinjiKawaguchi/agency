import AgencyAPI
import Foundation

/// How old a team's data is, when it is a copy from the team's node rather than the node's own.
public struct DataAge: Equatable, Sendable {
    public let text: String
    /// The copy could not be renewed, so what it shows may be out of date.
    public let isStale: Bool

    init(text: String, isStale: Bool) {
        self.text = text
        self.isStale = isStale
    }

    /// `nil` for a team on this node, whose data is current.
    public init?(team: Team, now: Date) {
        guard case .replica(let fetchedAt, _, let error) = team.source else { return nil }
        isStale = error != nil
        guard let fetchedAt else {
            text = Ja.Overview.neverRead(reason: error ?? "")
            return
        }
        let relative = Ago.text(from: fetchedAt.date, now: now)
        text = error.map { Ja.Overview.staleFrom(relative, reason: $0) } ?? Ja.Overview.dataFrom(relative)
    }
}
