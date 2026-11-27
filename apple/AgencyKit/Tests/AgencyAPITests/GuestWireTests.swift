@testable import AgencyAPI
import Foundation
import Testing

@Suite("Me.guest on the wire")
struct GuestWireTests {
    private func me(_ extra: String) throws -> Me {
        let json = #"{"account":{"id":"a","label":"A","fingerprint":"f"},"funding":[],"orgs":[]\#(extra)}"#
        return try JSONDecoder().decode(Me.self, from: Data(json.utf8))
    }

    @Test("a viewer the node does not call a guest is not one: the field left out, or null")
    func notAGuest() throws {
        #expect(try me("").guest == nil)
        #expect(try me(#","guest":null"#).guest == nil)
    }

    @Test("a hosted member is read with the member and the org the node names")
    func guest() throws {
        #expect(try me(#","guest":{"member":"erin","org":"acme"}"#).guest == GuestRef(member: "erin", org: "acme"))
    }

    @Test("any guest that is not null counts, whatever its shape: the viewer must not get the owner's menu and refusals")
    func failClosed() throws {
        for shape in [#"{}"#, #"{"member":"erin"}"#, #"{"org":7,"member":false}"#, #""erin""#, "true", "7", "[]", #"{"member":""}"#] {
            let guest = try me(#","guest":\#(shape)"#).guest
            #expect(guest != nil, Comment(rawValue: shape))
        }
        #expect(try me(#","guest":{"member":7,"org":"acme"}"#).guest == GuestRef(member: nil, org: "acme"), "a field that is not text is not read")
        #expect(try me(#","guest":"erin""#).guest == GuestRef(member: nil, org: nil))
    }
}
