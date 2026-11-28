import AgencyAPI
import AgencyKit
import Foundation

/// One complete reading of a node's desk with no screen and no session, for what runs in the background: the account and every
/// team once (no waiting for a team that is not ready: a background run has seconds).
@MainActor
enum DeskReading {
    /// The account and the teams, or `nil` when the desk could not be read in full: a number made of the teams that happened to
    /// answer would be wrong.
    static func complete(from source: any TeamSnapshotSource) async -> (me: Me, teams: [Team])? {
        guard let me = try? await source.me() else { return nil }
        let expected = me.orgs.reduce(0) { $0 + $1.teams.count }
        var teams: [Team] = []
        await TeamReader.read(me.orgs, from: source, retry: .never) { _, result in
            guard case .success(let snapshot) = result else { return }
            // A copy of a remote team's view that was never fetched holds nothing yet: that is not a read, and its 0
            // would be a number nothing explains.
            if case .replica(nil, _, _) = snapshot.source { return }
            teams.append(Team(snapshot: snapshot))
        }
        // A team that failed, was never fetched, or whose read was cancelled is missing from `teams`: that is an incomplete desk.
        guard teams.count == expected else { return nil }
        return (me, teams.sorted { $0.key < $1.key })
    }
}

/// How many requests wait for the owner on a node, read in the background (agency#117): what a wake that names another home node
/// does, so that the number on the icon holds that node's true count. It writes no widget file: the widgets show the open node.
@MainActor
public struct DeskWaitingCount {
    private let source: any TeamSnapshotSource

    public init(source: any TeamSnapshotSource) {
        self.source = source
    }

    /// The count without the items the owner has decided and the node has not taken yet, or `nil` when the desk could not be read in full.
    public func run(excluding decided: Set<String>) async -> Int? {
        await DeskReading.complete(from: source).map { Desk.waitingCount($0.teams, excluding: decided) }
    }
}

/// Which home node a background wake is for, against the nodes this device has.
public enum WakeTarget: Equatable, Sendable {
    /// The open node: a wake that names no node (an older node), or the open one's.
    case current
    /// Another node on this device.
    case other(HomeNodeURL)
    /// A node this device does not have: nothing to read.
    case unknown

    /// - Parameters:
    ///   - node: the id the push names (`NotificationTap.nodeID(in:)`).
    ///   - idOf: the id a push from a node carries (`HomeNodeBook.nodeID(of:)`).
    public static func resolve(node: String?, current: HomeNodeURL, others: [HomeNodeURL], idOf: (HomeNodeURL) -> String) -> WakeTarget {
        guard let node, node != idOf(current) else { return .current }
        return others.first { idOf($0) == node }.map(WakeTarget.other) ?? .unknown
    }
}
