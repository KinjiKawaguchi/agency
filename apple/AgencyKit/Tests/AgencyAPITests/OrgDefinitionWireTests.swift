import Foundation
import Testing
@testable import AgencyAPI

@Suite("Org definition wire type")
struct OrgDefinitionWireTests {
    @Test("the node's recorded definition decodes: its members, its team and no departments")
    func recorded() throws {
        let definition = try JSONDecoder().decode(OrgDefinition.self, from: Fixtures.data("org"))
        #expect(definition.org == "acme")
        #expect(definition.label == "Acme")
        #expect(definition.departments.isEmpty)
        #expect(definition.people.map(\.id) == ["alice", "bob"])
        #expect(definition.people[0].account == "alice")
        #expect(definition.people[0].department == nil)
        #expect(definition.people[0].role == .member)
        #expect(definition.teams == [TeamPlace(id: "app", label: "app", department: nil, node: "http://127.0.0.1:7701")])
    }

    @Test("departments nest by parent and name a head; a member and a team sit in a department")
    func departments() throws {
        let json = #"""
        {"org":"acme","label":"Acme","departments":[{"id":"eng","label":"Engineering","parent":null,"head":"alice"},{"id":"platform","label":"Platform","parent":"eng","head":null}],
         "people":[{"id":"alice","label":"Alice","account":"alice","department":"eng","role":"answerer"}],
         "teams":[{"id":"app","label":"App","department":"platform","node":"https://alice.example"}]}
        """#
        let definition = try JSONDecoder().decode(OrgDefinition.self, from: Data(json.utf8))
        #expect(definition.departments == [OrgDepartment(id: "eng", label: "Engineering", head: "alice"), OrgDepartment(id: "platform", label: "Platform", parent: "eng")])
        #expect(definition.people[0].role == .answerer)
        #expect(definition.teams[0].department == "platform")
    }

    @Test("a role the client does not know fails instead of being read as another")
    func unknownRole() {
        let json = #"{"org":"a","label":"A","departments":[],"people":[{"id":"x","label":"X","account":"x","department":null,"role":"owner"}],"teams":[]}"#
        #expect(throws: DecodingError.self) { try JSONDecoder().decode(OrgDefinition.self, from: Data(json.utf8)) }
    }
}
