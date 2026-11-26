import AgencyAPI
import AgencyKit
@testable import AgencyModel
import Foundation
import Testing

@Suite("HomeNodeList")
struct HomeNodeListTests {
    private func node(_ text: String) throws -> HomeNodeURL { try HomeNodeURL(validating: text) }
    private func added(_ text: String, _ label: String) throws -> HomeNodeEntry { HomeNodeEntry(url: try node(text), label: label, source: .added) }
    private func ref(_ label: String, _ url: String) -> HomeNodeRef { HomeNodeRef(label: label, url: url) }

    /// What the node says, decoded from its own fixture (`testdata/client/me-home-nodes.json`).
    private func fixtureNodes() throws -> [HomeNodeRef] {
        let me = try JSONDecoder().decode(Me.self, from: Data(try Fixture.data("me-home-nodes", from: URL(fileURLWithPath: #filePath).deletingLastPathComponent().path)))
        return me.homeNodes
    }

    @Test("the node's answer and an own entry spelled differently, with this node among them: two rows, in the node's order")
    func fixtureMerge() throws {
        let list = HomeNodeList.others(
            current: try node("https://agency.example.com"), configured: try fixtureNodes(),
            added: [try added("https://WORK.example.com:443/", "my work"), try added("agency.example.com", "this one")]
        )
        #expect(list.map(\.label) == ["Work", "Lab"])
        #expect(list.map { HomeNodeList.origin(of: $0.url) } == ["https://work.example.com:443", "http://127.0.0.1:4500"])
        #expect(list.map(\.source) == [.configured, .configured])
    }

    @Test("the node's entries first, then the owner's own, each origin once")
    func order() throws {
        let list = HomeNodeList.others(
            current: try node("https://a.example.com"),
            configured: [ref("Two", "https://b.example.com"), ref("Three", "https://c.example.com")],
            added: [try added("https://d.example.com", "Four"), try added("https://c.example.com", "mine"), try added("https://d.example.com:443", "again")]
        )
        #expect(list.map(\.label) == ["Two", "Three", "Four"])
        #expect(list.map(\.source) == [.configured, .configured, .added])
    }

    @Test("this node is never among the others, whichever way it is written")
    func notThisNode() throws {
        let list = HomeNodeList.others(
            current: try node("https://a.example.com"),
            configured: [ref("Itself", "https://a.example.com")], added: [try added("https://A.example.com:443", "me")]
        )
        #expect(list.isEmpty)
    }

    @Test("an entry the device cannot open is dropped, not trusted", arguments: [
        "ftp://x.example.com", "https://user:pw@x.example.com", "http://x.example.com", "", "not a url at all", "https://",
    ])
    func dropsUnopenable(url: String) throws {
        let list = HomeNodeList.others(current: try node("https://a.example.com"), configured: [ref("Bad", url), ref("Good", "https://g.example.com")], added: [])
        #expect(list.map(\.label) == ["Good"], "\(url)")
    }

    @Test("the node's label is shown whole, an own label is cut at 40, and an empty one is named by its host")
    func labels() throws {
        let long = String(repeating: "x", count: 80)
        let list = HomeNodeList.others(
            current: try node("https://a.example.com"), configured: [ref(long, "https://b.example.com"), ref("  ", "https://c.example.com")],
            added: [try added("https://d.example.com", long), try added("https://e.example.com", " Work ")]
        )
        #expect(list.map(\.label) == [long, "c.example.com", String(repeating: "x", count: 40), "Work"])
    }

    @Test("for anyone but the owner the node names none, and the list is the device's own")
    func notTheOwner() throws {
        let list = HomeNodeList.others(current: try node("https://a.example.com"), configured: [], added: [try added("https://b.example.com", "B")])
        #expect(list.map(\.label) == ["B"])
    }

    @Test("an entry that is passed as added but says it is configured is not listed twice or kept as the node's")
    func sourceOfAdded() throws {
        let wrong = HomeNodeEntry(url: try node("https://b.example.com"), label: "claims", source: .configured)
        #expect(HomeNodeList.others(current: try node("https://a.example.com"), configured: [], added: [wrong]).isEmpty)
    }
}
