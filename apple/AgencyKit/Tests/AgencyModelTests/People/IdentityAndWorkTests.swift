@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

@Suite("Faces")
struct IdentityTests {
    @Test("initials: a number keeps its letter and digits, capitals give two, a name gives two letters, 花子 gives one")
    func initials() {
        #expect(Identity.initials(of: "Build1") == "B1")
        #expect(Identity.initials(of: "build12") == "B12")
        #expect(Identity.initials(of: "TechLead") == "TL")
        #expect(Identity.initials(of: "Alice") == "Al")
        #expect(Identity.initials(of: "backend") == "Ba")
        #expect(Identity.initials(of: "x") == "X")
        #expect(Identity.initials(of: "花子") == "花")
        #expect(Identity.initials(of: "") == "")
    }

    @Test("a name with digits in the middle, or a digit first, is not a numbered name")
    func notNumbered() {
        #expect(Identity.initials(of: "a1b") == "A1")
        #expect(Identity.initials(of: "1abc") == "1a")
        #expect(Identity.initials(of: "12") == "12")
    }

    @Test("the hue is stable, in 1 to 8, and the same as the web client's for the same name")
    func hue() {
        #expect(Identity.hue(of: "Backend") == Identity.hue(of: "Backend"))
        for name in ["", "Lead", "Backend", "Frontend", "Docs", "Alice", "花子", "🚀"] {
            #expect((1...8).contains(Identity.hue(of: name)))
        }
        // The web: hash = (hash * 31 + codePoint) >>> 0, then hash % 8 + 1.
        #expect(Identity.hue(of: "a") == 97 % 8 + 1)
        #expect(Identity.hue(of: "ab") == (97 * 31 + 98) % 8 + 1)
    }
}

/// Ported from `web/src/people/work.test.ts`.
@Suite("Work and epics")
struct WorkTests {
    private func epic(_ ref: String = "app#1", total: Int = 0, done: Int = 0, doing: Int = 0, blocked: [(String, Int)] = []) -> EpicForecast {
        let reasons = blocked.map { #"{"reason":"\#($0.0)","count":\#($0.1)}"# }.joined(separator: ",")
        let json = #"{"ref":"\#(ref)","title":"Epic","total":\#(total),"done":\#(done),"doing":\#(doing),"remainingPoints":0,"blocked":[\#(reasons)],"basis":""}"#
        return try! JSONDecoder().decode(EpicForecast.self, from: Data(json.utf8))
    }

    @Test("splits the bar into done, doing, blocked and not started")
    func segments() {
        let sample = epic(total: 4, done: 1, doing: 1, blocked: [("needsDecision", 1), ("waitingOnPrevious", 1)])
        #expect(Work.segments(of: sample) == [.init(kind: .done, count: 1), .init(kind: .doing, count: 1), .init(kind: .blocked, count: 2)])
    }

    @Test("drops empty segments, and gives an epic with no children an empty bar")
    func emptySegments() {
        #expect(Work.segments(of: epic(total: 0)).isEmpty)
        #expect(Work.segments(of: epic(total: 5, done: 2)) == [.init(kind: .done, count: 2), .init(kind: .rest, count: 3)])
    }

    @Test("never shows a negative remainder when the counts overlap")
    func overlap() {
        #expect(!Work.segments(of: epic(total: 2, done: 2, doing: 1)).contains { $0.kind == .rest })
    }

    @Test("adds up the blocked reasons")
    func blockedTotal() {
        #expect(Work.blockedTotal(epic(blocked: [("external", 2), ("needsDecision", 1)])) == 3)
    }

    @Test("puts epics with work in progress first, then the furthest along, and keeps the team's order for a tie")
    func sorted() {
        let epics = [epic("a", total: 4, done: 1), epic("b", total: 4, done: 3), epic("c", total: 4, doing: 1), epic("d", total: 4, done: 1)]
        #expect(Work.sorted(epics).map(\.ref) == ["c", "b", "a", "d"])
    }

    @Test("a finished epic is sorted after every open epic, and an epic with a doneAt is finished however its counts read")
    func finishedEpics() throws {
        func epic(_ ref: String, total: Int, done: Int, doing: Int = 0, doneAt: String? = nil) throws -> EpicForecast {
            let stamp = doneAt.map { #","doneAt":"\#($0)""# } ?? ""
            let json = #"{"ref":"\#(ref)","title":"E","total":\#(total),"done":\#(done),"doing":\#(doing),"remainingPoints":0,"blocked":[],"basis":""\#(stamp)}"#
            return try JSONDecoder().decode(EpicForecast.self, from: Data(json.utf8))
        }
        let finished = try epic("old", total: 4, done: 4, doneAt: "2026-11-20T10:00:00Z")
        let open = try epic("open", total: 4, done: 1)
        let busy = try epic("busy", total: 4, done: 0, doing: 2)
        #expect(finished.isFinished)
        #expect(!open.isFinished)
        #expect(try epic("x", total: 4, done: 4).isFinished)
        #expect(try epic("x", total: 4, done: 1, doneAt: "2026-11-20T10:00:00Z").isFinished)
        #expect(try epic("x", total: 0, done: 0).isFinished == false)
        #expect(Work.sorted([finished, open, busy]).map(\.ref) == ["busy", "open", "old"])
        #expect(Work.sorted([finished, busy]).map(\.ref) == ["busy", "old"])
    }

    @Test("blocked work that is already in progress or finished is not listed as stuck")
    func blockedButStarted() throws {
        func item(_ ref: String, _ status: String, blocked: Bool) throws -> WorkItem {
            let stop = blocked ? #","blocked":{"reason":"external","since":"2026-11-20"}"# : ""
            return try JSONDecoder().decode(WorkItem.self, from: Data(#"{"ref":"\#(ref)","title":"t","status":"\#(status)"\#(stop)}"#.utf8))
        }
        let work = [try item("a#1", "inProgress", blocked: true), try item("a#2", "done", blocked: true), try item("a#3", "release", blocked: true), try item("a#4", "next", blocked: false)]
        #expect(Work.blocked(work).map(\.ref) == ["a#3"])
        #expect(Work.inProgress(work).map(\.ref) == ["a#1"])
    }

    @Test("separates what is in progress from what is blocked")
    func lists() {
        let sample = Teams.team().view.work
        #expect(Work.inProgress(sample).map(\.ref) == ["app#12"])
        #expect(Work.blocked(sample).map(\.ref) == ["app#14", "app#15"])
    }
}
