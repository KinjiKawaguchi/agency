import AgencyKit
@testable import AgencyModel
import Foundation
import Testing

@Suite("NotificationRouting and NodeID")
struct NotificationRoutingTests {
    private func node(_ text: String) throws -> HomeNodeURL { try HomeNodeURL(validating: text) }

    private static func file(from path: String = #filePath) throws -> [String: Any] {
        var url = URL(fileURLWithPath: path)
        for _ in 0..<6 { url.deleteLastPathComponent() }
        return try JSONSerialization.jsonObject(with: Data(contentsOf: url.appendingPathComponent("testdata/node-id.json"))) as! [String: Any]
    }

    @Test("the id of a node is the node's own: every case of the shared file, however the address is written")
    func vectors() throws {
        let cases = try Self.file()["cases"] as! [[String: String]]
        #expect(cases.count >= 5)
        for item in cases {
            #expect(NodeID.of(try node(item["url"]!)) == item["id"], Comment(rawValue: item["url"]!))
            #expect(NodeID.of(try node(item["origin"]!)) == item["id"], Comment(rawValue: item["origin"]!))
        }
    }

    @Test("an id is 32 lowercase hex characters, and nothing else is one")
    func shape() throws {
        #expect(NotificationTap.isNodeID(NodeID.of(try node("work.example.com"))))
        for odd in ["", "02AE509C6B20999646D3C600687EE1BF", "02ae509c6b20999646d3c600687ee1b", "02ae509c6b20999646d3c600687ee1bf0", "02ae509c6b20999646d3c600687ee1bg", "https://work.example.com"] {
            #expect(!NotificationTap.isNodeID(odd), Comment(rawValue: odd))
        }
    }

    @Test("a push that names no node, or the open one, opens here")
    func here() throws {
        let current = try node("a.example.com")
        let others = [try node("b.example.com")]
        #expect(NotificationRouting.action(forNode: nil, current: current, others: others) == .openHere)
        #expect(NotificationRouting.action(forNode: NodeID.of(try node("https://A.example.com:443/")), current: current, others: others) == .openHere)
    }

    @Test("a push from another node on this device switches to it, whichever way its address was typed")
    func switching() throws {
        let current = try node("a.example.com")
        let work = try node("https://Work.example.com:8443/")
        #expect(NotificationRouting.action(forNode: NodeID.of(try node("work.example.com:8443")), current: current, others: [try node("b.example.com"), work]) == .switchTo(work))
    }

    @Test("a push from a node this device does not have opens nothing")
    func unknown() throws {
        let current = try node("a.example.com")
        #expect(NotificationRouting.action(forNode: NodeID.of(try node("gone.example.com")), current: current, others: [try node("b.example.com")]) == .unknownNode)
        #expect(NotificationRouting.action(forNode: NodeID.of(try node("gone.example.com")), current: current, others: []) == .unknownNode)
    }

    @Test("a node is mapped by the id it said, not the one worked out from the address the app reaches it at")
    func saidID() throws {
        let current = try node("a.example.com")
        let loopback = try node("http://127.0.0.1:4500")
        let said = NodeID.of(try node("work.example.com"))
        var book = HomeNodeBook(current: current, added: [HomeNodeEntry(url: loopback, label: "L", source: .added)])
        book.note(nodeID: said, of: loopback)
        #expect(NotificationRouting.action(forNode: said, current: current, others: [loopback], idOf: book.nodeID(of:)) == .switchTo(loopback))
        #expect(NotificationRouting.action(forNode: NodeID.of(loopback), current: current, others: [loopback], idOf: book.nodeID(of:)) == .unknownNode,
                "the address it is reached at no longer names it once it has said its own")
    }

    @Test("the same host on another port is another node")
    func port() throws {
        let current = try node("a.example.com")
        #expect(NotificationRouting.action(forNode: NodeID.of(try node("a.example.com:8443")), current: current, others: []) == .unknownNode)
    }
}
