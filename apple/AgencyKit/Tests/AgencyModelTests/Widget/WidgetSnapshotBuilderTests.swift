@testable import AgencyAPI
@testable import AgencyModel
import AgencyWidget
import Foundation
import Testing

@Suite("Widget snapshot builder")
struct WidgetSnapshotBuilderTests {
    private let now = Date(timeIntervalSince1970: 1_795_521_600)

    @Test("the count is the requests that are the viewer's, and the list is the first three of them, in the desk's order")
    func mine() throws {
        let team = Teams.team()
        let items = Desk.openItems([team])
        let snapshot = WidgetSnapshotBuilder.snapshot(host: "node.example", items: items, now: now)
        #expect(snapshot.host == "node.example")
        #expect(snapshot.writtenAt == now)
        #expect(snapshot.waitingCount == items.filter(\.isMine).count)
        #expect(snapshot.waitingCount == team.mine.count)
        #expect(snapshot.requests.map(\.request) == items.filter(\.isMine).prefix(3).map(\.request.id))
        let first = try #require(snapshot.requests.first)
        #expect(first.org == "acme")
        #expect(first.team == "app")
        #expect(first.teamLabel == "App")
        #expect(first.title == items.filter(\.isMine)[0].request.title)
    }

    @Test("a request for someone else is not on the widget, and does not count")
    func others() {
        let team = Teams.team(mine: [Teams.questionID], addRequests: [Teams.request(to: "frontend")])
        let items = Desk.openItems([team])
        #expect(items.count > 1)
        let snapshot = WidgetSnapshotBuilder.snapshot(host: "h", items: items, now: now)
        #expect(snapshot.waitingCount == 1)
        #expect(snapshot.requests.map(\.request) == [Teams.questionID])
    }

    @Test("more than three waiting are counted all, and only three are listed")
    func many() {
        let extra = (1...5).map { Teams.request(id: "r-extra-\($0)", to: nil, title: "Request \($0)") }
        let team = Teams.team(addRequests: extra)
        let items = Desk.openItems([team])
        let snapshot = WidgetSnapshotBuilder.snapshot(host: "h", items: items, now: now)
        #expect(snapshot.waitingCount == items.filter(\.isMine).count)
        #expect(snapshot.waitingCount > 3)
        #expect(snapshot.requests.count == 3)
    }

    @Test("an empty desk gives a snapshot that says nothing waits")
    func empty() {
        let snapshot = WidgetSnapshotBuilder.snapshot(host: "h", items: [], now: now)
        #expect(snapshot.waitingCount == 0)
        #expect(snapshot.requests.isEmpty)
    }

    @Test("the kinds of request map to the widget's own kinds")
    func kinds() {
        let team = Teams.team()
        let items = Desk.openItems([team]).filter(\.isMine)
        let kinds = Set(items.map(\.request.kind))
        #expect(kinds.count >= 2)
        let snapshot = WidgetSnapshotBuilder.snapshot(host: "h", items: items, now: now)
        for (item, listed) in zip(items, snapshot.requests) {
            let expected: WidgetRequest.Kind = switch item.request.kind {
            case .proposal: .proposal
            case .question: .question
            case .pendingDecision: .pendingDecision
            }
            #expect(listed.kind == expected)
        }
    }
}
