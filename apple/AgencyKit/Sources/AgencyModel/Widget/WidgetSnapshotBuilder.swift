import AgencyAPI
import AgencyWidget
import Foundation

/// What the app writes for the widgets, worked out from the desk.
public enum WidgetSnapshotBuilder {
    /// The requests that are the viewer's to decide, in the desk's order, as the widget shows them. A request
    /// from someone else's pile is not listed: the number on the widget is the number on the desk's tab.
    /// - Parameter showsTitles: `false` leaves the title and the team's label out of the snapshot, so that nothing but
    ///   the kind of each request is in the file.
    public static func snapshot(host: String, items: [DeskItem], now: Date, showsTitles: Bool = true) -> WidgetSnapshot {
        let mine = items.filter(\.isMine)
        let listed = mine.prefix(WidgetSnapshot.requestLimit).map { item in
            WidgetRequest(
                org: item.org, team: item.team, request: item.request.id, kind: kind(item.request.kind),
                title: showsTitles ? item.request.title : nil, teamLabel: showsTitles ? item.teamLabel : nil
            )
        }
        return WidgetSnapshot(host: host, writtenAt: now, waitingCount: mine.count, requests: Array(listed))
    }

    private static func kind(_ kind: Request.Kind) -> WidgetRequest.Kind {
        switch kind {
        case .proposal: .proposal
        case .question: .question
        case .pendingDecision: .pendingDecision
        case .unknown: .other
        }
    }
}
