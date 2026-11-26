import Foundation
import Testing
@testable import AgencyKit

@Suite("BadgeLedger")
struct BadgeLedgerTests {
    private let a = "02ae509c6b20999646d3c600687ee1bf"
    private let b = "0881dc11d90faa5bd241e84dd481c251"
    private let c = "d6d3917538b937d9a7bde7c90d891b7b"

    @Test("the total is the sum of the last count of each node, and a node's new count replaces its old one")
    func sums() {
        var ledger = BadgeLedger(active: [a, b])
        #expect(ledger.total == 0)
        ledger = ledger.recording(3, for: a).recording(2, for: b)
        #expect(ledger.total == 5)
        ledger = ledger.recording(1, for: a)
        #expect(ledger.total == 3)
        #expect(ledger.recording(0, for: b).total == 1, "0 is a count: that node has nothing waiting")
    }

    @Test("a node this device does not have, and a count below 0, change nothing")
    func refuses() {
        let ledger = BadgeLedger(active: [a]).recording(2, for: a)
        #expect(ledger.recording(9, for: c) == ledger)
        #expect(ledger.recording(-1, for: a) == ledger)
    }

    @Test("a node that stops being the device's loses its count, and one that is left or forgotten stops counting")
    func leaves() {
        let ledger = BadgeLedger(active: [a, b]).recording(3, for: a).recording(2, for: b)
        #expect(ledger.activating([a]).total == 3)
        #expect(ledger.activating([a]).counts[b] == nil)
        #expect(ledger.forgetting(a).total == 2)
        #expect(ledger.forgetting(a).counts[a] == nil, "forgotten, not set to 0")
        #expect(ledger.forgetting(a).active == [a, b], "it stays one of the device's nodes: its next count counts")
        #expect(BadgeLedger(counts: [a: 4, c: 9], active: [a]).total == 4, "a count of a node that is not active is not kept")
        #expect(BadgeLedger(counts: [a: 4, b: -3], active: [a, b]).counts == [a: 4], "a count below 0 is not kept")
    }

    private func directory() throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("badge-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    @Test("it is kept in a file and read back, a missing or damaged file is an empty ledger")
    func file() throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let file = BadgeLedgerFile(directory: directory)
        #expect(file.load() == BadgeLedger())
        let written = file.update { $0.activating([a, b]).recording(3, for: a) }
        #expect(written.total == 3)
        #expect(file.load() == written)
        try Data("not json".utf8).write(to: directory.appendingPathComponent(BadgeLedgerFile.fileName))
        #expect(file.load() == BadgeLedger())
    }

    private func payload(node: String?) -> [AnyHashable: Any] {
        var keys: [String: Any] = ["org": "acme", "team": "app", "request": "r-1"]
        if let node { keys["node"] = node }
        return ["aps": ["badge": 2], "agency": keys]
    }

    @Test("a push of a node this device has sets the icon to the sum, and keeps the node's count")
    func pushSums() throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let file = BadgeLedgerFile(directory: directory)
        file.update { $0.activating([a, b]).recording(3, for: a) }
        #expect(BadgeFromPush.badge(userInfo: payload(node: b), pushBadge: 2, file: file) == 5)
        #expect(file.load().counts[b] == 2)
        #expect(BadgeFromPush.badge(userInfo: payload(node: b), pushBadge: 0, file: file) == 3, "the node has none waiting now")
    }

    @Test("a push that names no node, has no count, or comes from a node this device does not have, is left as it came")
    func pushLeft() throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let file = BadgeLedgerFile(directory: directory)
        file.update { $0.activating([a]).recording(3, for: a) }
        let before = file.load()
        #expect(BadgeFromPush.badge(userInfo: payload(node: nil), pushBadge: 2, file: file) == nil)
        #expect(BadgeFromPush.badge(userInfo: payload(node: a), pushBadge: nil, file: file) == nil)
        #expect(BadgeFromPush.badge(userInfo: payload(node: c), pushBadge: 2, file: file) == nil)
        #expect(BadgeFromPush.badge(userInfo: payload(node: "work.example.com"), pushBadge: 2, file: file) == nil)
        #expect(file.load() == before, "nothing was recorded")
    }

    @Test("with nothing written by the app yet, a push is left as it came")
    func noLedger() throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        #expect(BadgeFromPush.badge(userInfo: payload(node: a), pushBadge: 2, file: BadgeLedgerFile(directory: directory)) == nil)
    }
}
