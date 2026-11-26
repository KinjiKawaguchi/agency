#if canImport(SwiftUI)
import AgencyModel
import Foundation
import Testing
@testable import AgencyUI

@MainActor
@Suite("A request row for a seat or an external party")
struct AnswerForRowTests {
    private let now = UIFixtures.now

    @Test("the row says whom the viewer answers it for, after the team and before the time, and says nothing without it")
    func meta() {
        let seat = UIFixtures.request(id: "r-seat", title: "Pay the invoice?")
        let team = UIFixtures.team(extra: [seat], mine: ["r-seat"], relayedFor: ["r-seat": "Dan (tax)"])
        let line = RequestRow.meta(of: UIFixtures.item("r-seat", in: team), now: now)
        #expect(line.contains(" · Dan (tax) の代理で答える · "))
        let plain = UIFixtures.team(extra: [seat], mine: ["r-seat"])
        #expect(!RequestRow.meta(of: UIFixtures.item("r-seat", in: plain), now: now).contains("の代理で答える"))
        let emptyLabel = UIFixtures.team(extra: [seat], mine: ["r-seat"], relayedFor: ["r-seat": ""])
        #expect(!RequestRow.meta(of: UIFixtures.item("r-seat", in: emptyLabel), now: now).contains("の代理で答える"))
    }

    @Test("a label that looks like markup or has no break is shown as the text it is")
    func oddLabel() {
        let seat = UIFixtures.request(id: "r-seat", title: "Pay the invoice?")
        let odd = "<b>Dan</b> **x** " + String(repeating: "x", count: 70)
        let team = UIFixtures.team(extra: [seat], mine: ["r-seat"], relayedFor: ["r-seat": odd])
        #expect(RequestRow.meta(of: UIFixtures.item("r-seat", in: team), now: now).contains("\(odd) の代理で答える"))
    }
}
#endif
