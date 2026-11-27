@testable import AgencyModel
import Foundation
import Testing

@Suite("The item after a decision")
struct NextItemTests {
    private func items() -> [DeskItem] {
        Desk.openItems([Teams.team(addRequests: [Teams.request(id: "r-others", to: "frontend")])])
    }

    @Test("the next of the viewer's own after the one decided, in the pile's order, and from the top when none is after it")
    func next() throws {
        let pile = items().filter(\.isMine)
        #expect(pile.count >= 2)
        #expect(Desk.next(after: pile[0].id, in: pile) == pile[1].id)
        #expect(Desk.next(after: pile[pile.count - 1].id, in: pile) == pile[0].id, "wraps to the top")
    }

    @Test("what is addressed to someone else is not offered, and with nothing else of theirs there is nothing to move to")
    func others() throws {
        let all = items()
        let others = try #require(all.first { !$0.isMine })
        let mine = all.filter(\.isMine)
        #expect(Desk.next(after: mine[0].id, in: all) != others.id)
        let one = [mine[0]] + all.filter { !$0.isMine }
        #expect(Desk.next(after: mine[0].id, in: one) == nil)
    }

    @Test("an item that is already off the pile (decided) still has a next")
    func decidedAlreadyGone() throws {
        let mine = items().filter(\.isMine)
        let remaining = Array(mine.dropFirst())
        #expect(Desk.next(after: mine[0].id, in: remaining) == remaining[0].id)
    }

    @Test("decided items are left out of the pile and the count")
    func excluding() {
        let teams = [Teams.team()]
        let all = Desk.openItems(teams)
        let decided: Set<String> = [all[0].id]
        #expect(Desk.openItems(teams, excluding: decided).map(\.id) == all.dropFirst().map(\.id))
        #expect(Desk.waitingCount(teams, excluding: decided) == Desk.waitingCount(teams) - (all[0].isMine ? 1 : 0))
    }
}
