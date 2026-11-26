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

    @Test("the node's recorded answers do not hide titles")
    func recorded() throws {
        for name in ["me", "me-member", "me-funding"] {
            #expect(try !JSONDecoder().decode(Me.self, from: Fixtures.data(name)).hidesTitles, Comment(rawValue: name))
        }
    }

    @Test("a value that is not a boolean refuses the answer instead of being read as false")
    func notBoolean() {
        #expect(throws: DecodingError.self) { try me(#","hidesTitles":"yes""#) }
    }
}
