import AgencyAPI
import AgencyKit
@testable import AgencyModel
import Foundation
import Testing

@Suite("HomeNodeBook")
struct HomeNodeBookTests {
    private func node(_ text: String) throws -> HomeNodeURL { try HomeNodeURL(validating: text) }
    private func hosts(_ entries: [HomeNodeEntry]) -> [String] { entries.map(\.url.host) }

    @Test("the first address becomes the current node and the list has nothing else")
    func first() throws {
        var book = HomeNodeBook()
        #expect(book.current == nil && book.others.isEmpty)
        book.switchTo(try node("a.example.com"))
        #expect(book.current?.host == "a.example.com")
        #expect(book.others.isEmpty)
    }

    @Test("going to another node keeps the one that was current on the list, and the one gone to leaves it")
    func switching() throws {
        var book = HomeNodeBook(current: try node("a.example.com"))
        book.switchTo(try node("b.example.com"))
        #expect(book.current?.host == "b.example.com")
        #expect(hosts(book.others) == ["a.example.com"])
        book.switchTo(try node("a.example.com"))
        #expect(book.current?.host == "a.example.com")
        #expect(hosts(book.others) == ["b.example.com"])
        book.switchTo(try node("https://A.example.com:443/"))
        #expect(hosts(book.others) == ["b.example.com"], "the same node written differently is not a switch")
    }

    @Test("an added node is listed once, not as the current node, and in the order added")
    func adding() throws {
        var book = HomeNodeBook(current: try node("a.example.com"))
        book.add(try node("b.example.com"), label: "B")
        book.add(try node("c.example.com"), label: "C")
        book.add(try node("https://B.example.com:443"), label: "again")
        book.add(try node("a.example.com"), label: "itself")
        #expect(book.others.map(\.label) == ["B", "C"])
        #expect(book.others.map(\.source) == [.added, .added])
    }

    @Test("what the current node says is listed first, wins over an own entry of the same node, and goes when the node does")
    func configured() throws {
        var book = HomeNodeBook(current: try node("a.example.com"))
        book.add(try node("b.example.com"), label: "mine")
        book.note(configured: [HomeNodeRef(label: "Work", url: "https://b.example.com"), HomeNodeRef(label: "Lab", url: "http://127.0.0.1:4500")])
        #expect(book.others.map(\.label) == ["Work", "Lab"])
        #expect(book.others.map(\.source) == [.configured, .configured])
        book.switchTo(try node("b.example.com"))
        #expect(book.configured.isEmpty, "another node's word about its owner's nodes is not this node's")
        #expect(hosts(book.others) == ["a.example.com"])
    }

    @Test("forgetting takes a node off the list and leaves the current one alone")
    func forgetting() throws {
        var book = HomeNodeBook(current: try node("a.example.com"))
        book.add(try node("b.example.com"), label: "B")
        book.add(try node("c.example.com"), label: "C")
        book.forget(try node("b.example.com"))
        #expect(book.others.map(\.label) == ["C"])
        book.forget(try node("a.example.com"))
        #expect(book.current?.host == "a.example.com")
    }

    @Test("leaving the current node leaves nothing current, forgets what it said, and keeps it on the list to be forgotten")
    func leaving() throws {
        var book = HomeNodeBook(current: try node("a.example.com"))
        book.add(try node("b.example.com"), label: "B")
        book.note(configured: [HomeNodeRef(label: "Work", url: "https://w.example.com")])
        book.leaveCurrent()
        #expect(book.current == nil && book.configured.isEmpty)
        #expect(book.others.map(\.label) == ["B", "a.example.com"])
        #expect(book.forget(try node("a.example.com")))
        #expect(hosts(book.others) == ["b.example.com"])
    }

    @Test("what is kept for a host goes only when no other known node has that host, and never for the current node")
    func forgettingByHost() throws {
        var book = HomeNodeBook(current: try node("https://a.example.com"))
        book.add(try node("https://a.example.com:8443"), label: "same host")
        book.add(try node("https://b.example.com"), label: "B")
        book.add(try node("https://c.example.com"), label: "C")
        book.note(configured: [HomeNodeRef(label: "Named", url: "https://c.example.com:8444")])
        #expect(book.forget(try node("https://a.example.com:8443")) == false, "the current node has the host")
        #expect(book.forget(try node("https://b.example.com")), "nothing else has it")
        #expect(book.forget(try node("https://c.example.com")) == false, "a node the current one names has the host")
        #expect(book.forget(try node("https://a.example.com")) == false, "the current node is not forgotten")
        #expect(book.current?.host == "a.example.com")
        #expect(hosts(book.others) == ["c.example.com"])
    }

    @Test("a node that is only named by the current node is not forgotten: nothing changes and nothing may go")
    func forgettingANamedNode() throws {
        var book = HomeNodeBook(current: try node("a.example.com"))
        book.note(configured: [HomeNodeRef(label: "Named", url: "https://w.example.com")])
        let before = book
        #expect(book.forget(try node("w.example.com")) == false)
        #expect(book == before)
        #expect(hosts(book.others) == ["w.example.com"])
    }

    @Test("a node the owner added that the current node also names: forgetting it clears the device's data and the named row stays")
    func forgettingAnAddedNodeThatIsAlsoNamed() throws {
        var book = HomeNodeBook(current: try node("a.example.com"))
        book.add(try node("https://w.example.com"), label: "mine")
        book.note(configured: [HomeNodeRef(label: "Named", url: "https://w.example.com")])
        #expect(book.forget(try node("https://w.example.com")))
        #expect(book.others.map(\.label) == ["Named"])
    }

    @Test("the current node is never forgotten, whatever else is added")
    func currentIsNeverForgotten() throws {
        var book = HomeNodeBook(current: try node("a.example.com"))
        let before = book
        #expect(book.forget(try node("https://A.example.com:443/")) == false)
        #expect(book == before)
    }

    @Test("two added nodes of one host: the first forgotten keeps the host's data, the last takes it")
    func forgettingTwoOfAHost() throws {
        var book = HomeNodeBook(current: try node("https://a.example.com"))
        book.add(try node("https://b.example.com"), label: "B1")
        book.add(try node("https://b.example.com:8443"), label: "B2")
        #expect(book.forget(try node("https://b.example.com")) == false)
        #expect(book.forget(try node("https://b.example.com:8443")))
    }

    @Test("it is kept and read back as it was, and what cannot be opened any more is dropped")
    func keeping() throws {
        var book = HomeNodeBook(current: try node("a.example.com"))
        book.add(try node("b.example.com"), label: "B")
        book.note(configured: [HomeNodeRef(label: "Work", url: "https://w.example.com")])
        #expect(HomeNodeBook(decoding: book.encoded(), legacyAddress: nil) == book)
        let broken = #"{"current":"https://a.example.com","added":[{"url":"ftp://x","label":"X"},{"url":"https://b.example.com","label":"B"}],"configured":[]}"#
        let read = HomeNodeBook(decoding: Data(broken.utf8), legacyAddress: nil)
        #expect(hosts(read.others) == ["b.example.com"])
    }

    @Test("what a node says of its own id is kept for it, replaced by what it says later, forgotten when it says none, and not kept when it is not an id")
    func nodeIDs() throws {
        let a = try node("https://a.example.com")
        var book = HomeNodeBook(current: a)
        #expect(book.nodeID(of: a) == NodeID.of(a), "worked out from the address until it has said")
        #expect(!book.hasSaidID(of: a))
        let said = "02ae509c6b20999646d3c600687ee1bf"
        book.note(nodeID: said, of: a)
        #expect(book.nodeID(of: a) == said)
        #expect(book.hasSaidID(of: a))
        #expect(book.hasSaidID(of: try node("https://A.example.com:443/")))
        #expect(!book.hasSaidID(of: try node("https://b.example.com")))
        #expect(book.nodeID(of: try node("https://A.example.com:443/")) == said, "however the address is written")
        #expect(HomeNodeBook(decoding: book.encoded(), legacyAddress: nil).nodeID(of: a) == said, "kept with the book")
        book.note(nodeID: "not an id", of: a)
        #expect(book.nodeID(of: a) == NodeID.of(a))
        book.note(nodeID: said, of: a)
        book.note(nodeID: nil, of: a)
        #expect(book.nodeID(of: a) == NodeID.of(a))
    }

    @Test("an id that is not one is not read back from what was kept")
    func keptJunkID() throws {
        let a = try node("https://a.example.com")
        let kept = HomeNodeBook(current: a, nodeIDs: [a.origin: "work.example.com"])
        #expect(kept.nodeID(of: a) == NodeID.of(a))
    }

    @Test("forgetting a node forgets the id it said, and a book kept before ids were kept reads as having none")
    func nodeIDsForgotten() throws {
        let b = try node("https://b.example.com")
        var book = HomeNodeBook(current: try node("https://a.example.com"))
        book.add(b, label: "B")
        book.note(nodeID: "02ae509c6b20999646d3c600687ee1bf", of: b)
        book.forget(b)
        #expect(book.nodeID(of: b) == NodeID.of(b))
        let older = #"{"current":"https://a.example.com","added":[],"configured":[]}"#
        #expect(HomeNodeBook(decoding: Data(older.utf8), legacyAddress: nil).nodeIDs.isEmpty)
    }

    @Test("an earlier version's one address becomes the current node, and an unreadable book falls back to it")
    func legacy() throws {
        #expect(HomeNodeBook(decoding: nil, legacyAddress: "https://a.example.com").current?.host == "a.example.com")
        #expect(HomeNodeBook(decoding: Data("not json".utf8), legacyAddress: "https://a.example.com").current?.host == "a.example.com")
        #expect(HomeNodeBook(decoding: nil, legacyAddress: nil).current == nil)
        #expect(HomeNodeBook(decoding: nil, legacyAddress: "ftp://nope").current == nil)
    }

    @Test("an own label is cut at 40 and a node that was current is listed by its host")
    func labels() throws {
        var book = HomeNodeBook(current: try node("a.example.com"))
        book.add(try node("b.example.com"), label: String(repeating: "x", count: 80))
        book.switchTo(try node("c.example.com"))
        #expect(book.others.map(\.label) == [String(repeating: "x", count: 40), "a.example.com"])
    }

    @Test("what is kept never holds the current node, a node twice, or an entry that is not the owner's own")
    func keptList() throws {
        var book = HomeNodeBook(current: try node("a.example.com"))
        book.add(try node("a.example.com"), label: "itself")
        book.add(try node("b.example.com"), label: "B")
        book.add(try node("b.example.com"), label: "again")
        #expect(hosts(book.added) == ["b.example.com"])
        book.switchTo(try node("b.example.com"))
        #expect(hosts(book.added) == ["a.example.com"], "the node gone to leaves the kept list and the one left joins it")
        let given = [
            HomeNodeEntry(url: try node("a.example.com"), label: "current", source: .added),
            HomeNodeEntry(url: try node("c.example.com"), label: "C", source: .configured),
            HomeNodeEntry(url: try node("d.example.com"), label: "D", source: .added),
        ]
        let made = HomeNodeBook(current: try node("a.example.com"), added: given)
        #expect(hosts(made.added) == ["d.example.com"])
    }
}
