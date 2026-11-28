#if canImport(SwiftUI)
import AgencyModel
import Foundation
import Testing
@testable import AgencyUI

@MainActor
@Suite("A request row for a seat or an external party")
struct AnswerForRowTests {
    private let now = UIFixtures.now
    private let seat = UIFixtures.request(id: "r-seat", title: "Pay the invoice?")

    private func item(_ label: String?) -> DeskItem {
        let team = UIFixtures.team(extra: [seat], mine: ["r-seat"], relayedFor: label.map { ["r-seat": $0] })
        return UIFixtures.item("r-seat", in: team)
    }

    @Test("the row has a line of its own for whom the viewer answers for, and the meta line, which is cut after two lines, holds none of it")
    func line() {
        #expect(RequestRow.answerLine(of: item("Dan (tax)")) == "Dan (tax) の代理回答")
        #expect(!RequestRow.meta(of: item("Dan (tax)"), now: now).contains("Dan"))
        #expect(!RequestRow.meta(of: item("Dan (tax)"), now: now).contains("の代理回答"))
    }

    @Test("no line without a label, with an empty one, or with only spaces; the spaces around a label are not shown")
    func absent() {
        #expect(RequestRow.answerLine(of: item(nil)) == nil)
        #expect(RequestRow.answerLine(of: item("")) == nil)
        #expect(RequestRow.answerLine(of: item("   ")) == nil)
        #expect(RequestRow.answerLine(of: item(" \n\t ")) == "?? の代理回答", "control characters are shown as ?, so the label is not empty")
        #expect(RequestRow.answerLine(of: item("  Dan (tax) ")) == "Dan (tax) の代理回答")
    }

    @Test("a seat's request of a team on another node, for a reader the team does not name, has its line too")
    func remoteTeam() {
        let replica: [String: Any] = ["kind": "replica", "fetchedAt": "2026-11-26T09:00:00Z", "triedAt": "2026-11-26T09:00:30Z"]
        let team = UIFixtures.team(extra: [seat], mine: ["r-seat"], relayedFor: ["r-seat": "Dan (tax)"], viewer: nil, source: replica)
        #expect(RequestRow.answerLine(of: UIFixtures.item("r-seat", in: team)) == "Dan (tax) の代理回答")
    }

    @Test("a label that looks like markup or has no break is shown as the text it is, whole")
    func oddLabel() {
        let odd = "<b>Dan</b> **x** " + String(repeating: "x", count: 70)
        #expect(RequestRow.answerLine(of: item(odd)) == "\(odd) の代理回答")
    }
}
#endif
