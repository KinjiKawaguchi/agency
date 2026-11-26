import Foundation
import Testing
@testable import AgencyAPI

@Suite("Me: a node that hides titles")
struct MeHidesTitlesTests {
    private func me(_ extra: String) throws -> Me {
        let json = #"{"account":{"id":"alice","label":"A","fingerprint":"f"},"funding":[],"orgs":[]\#(extra)}"#
        return try JSONDecoder().decode(Me.self, from: Data(json.utf8))
    }

    @Test("a node that does not say counts as not hiding titles; one that says so hides them")
    func hides() throws {
        #expect(try !me("").hidesTitles)
        #expect(try !me(#","hidesTitles":false"#).hidesTitles)
        #expect(try me(#","hidesTitles":true"#).hidesTitles)
    }

    @Test("the node's recorded answers leave the field out, and are read as not hiding titles")
    func recorded() throws {
        for name in ["me", "me-member", "me-funding"] {
            #expect(try !Fixtures.text(name).contains("hidesTitles"), Comment(rawValue: name))
            #expect(try !JSONDecoder().decode(Me.self, from: Fixtures.data(name)).hidesTitles, Comment(rawValue: name))
        }
    }

    @Test("the answer of a node that hides titles says so, and the rest of it is read as usual")
    func recordedHiding() throws {
        #expect(try Fixtures.text("me-hides-titles").contains(#""hidesTitles": true"#))
        let hiding = try JSONDecoder().decode(Me.self, from: Fixtures.data("me-hides-titles"))
        let plain = try JSONDecoder().decode(Me.self, from: Fixtures.data("me"))
        #expect(hiding.hidesTitles)
        #expect(hiding.account == plain.account)
        #expect(hiding.orgs == plain.orgs)
        #expect(hiding.funding == plain.funding)
    }

    @Test("a value that is not a boolean refuses the answer instead of being read as false")
    func notBoolean() {
        #expect(throws: DecodingError.self) { try me(#","hidesTitles":"yes""#) }
    }
}
