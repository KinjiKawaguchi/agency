import Foundation
import Testing
@testable import AgencyKit

@Suite("DeskWake")
struct DeskWakeTests {
    @Test("a push with content-available 1 is a wake, whether it is the node's desk push or a request's alert")
    func wakes() {
        #expect(DeskWake.isWake(["aps": ["content-available": 1], "agency": ["type": "desk"]]))
        #expect(DeskWake.isWake(["aps": ["alert": ["title": "app", "body": "x"], "content-available": 1], "agency": ["org": "acme"]]))
        #expect(DeskWake.isWake(["aps": ["content-available": NSNumber(value: 1)]]))
    }

    @Test("anything else is not: no flag, 0, another number, text, no aps, or an aps that is not a dictionary")
    func doesNotWake() {
        let others: [[String: Any]] = [
            ["aps": ["alert": "x"]], ["aps": ["content-available": 0]], ["aps": ["content-available": 2]],
            ["aps": ["content-available": "1"]], ["agency": ["type": "desk"]], ["aps": "content-available"],
        ]
        for userInfo in others {
            #expect(!DeskWake.isWake(userInfo), "\(userInfo)")
        }
    }

    @Test("a reading is due when there was none, and a minute after the last")
    func due() {
        let now = Date(timeIntervalSince1970: 1_795_521_600)
        #expect(DeskWake.isDue(lastRun: nil, now: now))
        #expect(!DeskWake.isDue(lastRun: now.addingTimeInterval(-59), now: now))
        #expect(DeskWake.isDue(lastRun: now.addingTimeInterval(-60), now: now))
        #expect(DeskWake.isDue(lastRun: now.addingTimeInterval(-3600), now: now))
        #expect(!DeskWake.isDue(lastRun: now, now: now))
    }
}
