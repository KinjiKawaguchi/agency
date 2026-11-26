import AgencyKit
@testable import AgencyModel
import Foundation
import Testing

@Suite("WakeTarget")
struct WakeTargetTests {
    private func node(_ text: String) throws -> HomeNodeURL { try HomeNodeURL(validating: text) }

    @Test("a wake that names no node, or the open one, is for the open node")
    func current() throws {
        let a = try node("a.example.com")
        let others = [try node("b.example.com")]
        #expect(WakeTarget.resolve(node: nil, current: a, others: others, idOf: NodeID.of) == .current)
        #expect(WakeTarget.resolve(node: NodeID.of(try node("https://A.example.com:443/")), current: a, others: others, idOf: NodeID.of) == .current)
    }

    @Test("a wake that names another node of this device is for that node, by the id the node said or the one worked out")
    func other() throws {
        let a = try node("a.example.com")
        let loopback = try node("http://127.0.0.1:4500")
        let said = NodeID.of(try node("work.example.com"))
        let idOf: (HomeNodeURL) -> String = { $0 == loopback ? said : NodeID.of($0) }
        #expect(WakeTarget.resolve(node: said, current: a, others: [loopback], idOf: idOf) == .other(loopback))
        #expect(WakeTarget.resolve(node: NodeID.of(loopback), current: a, others: [loopback], idOf: idOf) == .unknown)
    }

    @Test("a wake from a node this device does not have is for nobody")
    func unknown() throws {
        let a = try node("a.example.com")
        #expect(WakeTarget.resolve(node: NodeID.of(try node("gone.example.com")), current: a, others: [try node("b.example.com")], idOf: NodeID.of) == .unknown)
    }
}
