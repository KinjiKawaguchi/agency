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

    @Test("leaving the current node leaves nothing current, forgets what it said, and keeps the owner's own list")
    func leaving() throws {
        var book = HomeNodeBook(current: try node("a.example.com"))
        book.add(try node("b.example.com"), label: "B")
        book.note(configured: [HomeNodeRef(label: "Work", url: "https://w.example.com")])
        book.leaveCurrent()
        #expect(book.current == nil && book.configured.isEmpty)
        #expect(book.others.map(\.label) == ["B"])
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
