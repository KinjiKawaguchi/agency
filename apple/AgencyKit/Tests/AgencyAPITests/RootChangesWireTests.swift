@testable import AgencyAPI
import Foundation
import Testing

@Suite("Held root changes on the wire")
struct RootChangesWireTests {
    private func me(_ extra: String) throws -> Me {
        let json = #"{"account":{"id":"alice","label":"A","fingerprint":"f"},"funding":[],"orgs":[]\#(extra)}"#
        return try JSONDecoder().decode(Me.self, from: Data(json.utf8))
    }

    private let held = #"{"id":"c1","org":"acme","what":"add member carol (Carol)","receivedAt":"2026-11-26T09:00:00Z"}"#

    @Test("a node that names none, or says the list is empty, leaves the owner with none")
    func none() throws {
        #expect(try me("").rootChanges.isEmpty)
        #expect(try me(#","rootChanges":[]"#).rootChanges.isEmpty)
    }

    @Test("a held change reads with its id, org, words and time, and without the fields a node of today does not give")
    func reads() throws {
        let change = try #require(try me(#","rootChanges":[\#(held)]"#).rootChanges.first)
        #expect(change == RootChangeRef(id: "c1", org: "acme", what: "add member carol (Carol)", receivedAt: "2026-11-26T09:00:00Z"))
        #expect(change.kind == nil && change.member == nil && change.label == nil && change.device == nil)
    }

    @Test("the kind and the fields it is worded from are read when the node gives them, and an empty one is none")
    func fields() throws {
        let json = #"{"id":"c2","org":"acme","receivedAt":"2026-11-26T09:00:00Z","kind":"addDevice","member":"bob","label":"iPhone","device":""}"#
        let change = try #require(try me(#","rootChanges":[\#(json)]"#).rootChanges.first)
        #expect(change.kind == "addDevice" && change.member == "bob" && change.label == "iPhone")
        #expect(change.device == nil && change.what == nil)
    }

    @Test("an entry that is not a held change is left out, and the account still reads with the others")
    func lossy() throws {
        let entries = [#"{"id":"","org":"acme","what":"x","receivedAt":"t"}"#, #"{"org":"acme","what":"x","receivedAt":"t"}"#, #"7"#, #"{"id":"c3","org":"acme","receivedAt":""}"#, held]
        #expect(try me(#","rootChanges":[\#(entries.joined(separator: ","))]"#).rootChanges.map(\.id) == ["c1"])
    }

    @Test("a list that is not a list is none, and does not make the account unreadable")
    func notAList() throws {
        #expect(try me(#","rootChanges":"later""#).rootChanges.isEmpty)
        #expect(try me(#","rootChanges":{"id":"c1"}"#).rootChanges.isEmpty)
    }

    @Test("a device change's outcome reads for the ones this app knows, the held one, and one it does not know")
    func outcomes() throws {
        func result(_ json: String) throws -> DeviceChangeResult { try JSONDecoder().decode(DeviceChangeResult.self, from: Data(json.utf8)) }
        #expect(try result(#"{"outcome":"applied"}"#).outcome == .applied)
        #expect(try result(#"{"outcome":"delivered"}"#).outcome == .delivered)
        #expect(try result(#"{"outcome":"queued","message":"asleep"}"#) == DeviceChangeResult(outcome: .queued, message: "asleep"))
        #expect(try result(#"{"outcome":"pending","message":"Approved here"}"#) == DeviceChangeResult(outcome: .pending, message: "Approved here"))
        #expect(try result(#"{"outcome":"later","message":"m"}"#) == DeviceChangeResult(outcome: .unknown("later"), message: "m"))
    }
}
