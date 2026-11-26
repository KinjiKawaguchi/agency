import AgencyKit
@testable import AgencyWidget
import Foundation
import Testing

private func request(_ id: String = "r-1", title: String = "Ship it?", team: String = "App", kind: WidgetRequest.Kind = .question) -> WidgetRequest {
    WidgetRequest(org: "acme", team: "app", request: id, kind: kind, title: title, teamLabel: team)
}

private let written = Date(timeIntervalSince1970: 1_795_521_600)

@Suite("Widget snapshot")
struct WidgetSnapshotTests {
    @Test("lists the first three requests only, whatever it is given, and never a negative count")
    func limits() {
        let many = (1...6).map { request("r-\($0)") }
        let snapshot = WidgetSnapshot(host: "node.example", writtenAt: written, waitingCount: 6, requests: many)
        #expect(snapshot.requests.map(\.request) == ["r-1", "r-2", "r-3"])
        #expect(snapshot.waitingCount == 6)
        #expect(WidgetSnapshot(host: "h", writtenAt: written, waitingCount: -2, requests: []).waitingCount == 0)
        #expect(snapshot.version == WidgetSnapshot.currentVersion)
    }

    @Test("a long title is cut at a character with an ellipsis, a short one is kept, and spaces around it go")
    func cutting() {
        let long = String(repeating: "あ", count: 200)
        let cut = request(title: long).title ?? ""
        #expect(cut.count == WidgetRequest.titleLimit)
        #expect(cut.hasSuffix("…"))
        #expect(request(title: "  Ship it?\n").title == "Ship it?")
        #expect(request(title: String(repeating: "x", count: WidgetRequest.titleLimit)).title?.count == WidgetRequest.titleLimit)
        #expect(request(title: String(repeating: "x", count: WidgetRequest.titleLimit)).title?.hasSuffix("…") == false)
        #expect(request(team: String(repeating: "t", count: 100)).teamLabel?.count == WidgetRequest.teamLimit)
        // An emoji outside the BMP is one character, not two halves.
        #expect(request(title: String(repeating: "🚀", count: 200)).title?.count == WidgetRequest.titleLimit)
    }

    @Test("a snapshot is fresh for three hours, then says how old it is")
    func staleness() {
        let snapshot = WidgetSnapshot(host: "h", writtenAt: written, waitingCount: 1, requests: [])
        #expect(snapshot.staleness(now: written) == .fresh)
        #expect(snapshot.staleness(now: written.addingTimeInterval(3 * 3600 - 1)) == .fresh)
        #expect(snapshot.staleness(now: written.addingTimeInterval(3 * 3600)) == .stale(minutes: 180))
        #expect(snapshot.staleness(now: written.addingTimeInterval(26 * 3600)) == .stale(minutes: 26 * 60))
        // A clock that went back is not "old".
        #expect(snapshot.staleness(now: written.addingTimeInterval(-60)) == .fresh)
    }

    @Test("two snapshots say the same when the node, the count and the requests are the same, whenever they were written")
    func sameContent() {
        let first = WidgetSnapshot(host: "h", writtenAt: written, waitingCount: 2, requests: [request("r-1")])
        #expect(first.hasSameContent(as: WidgetSnapshot(host: "h", writtenAt: written.addingTimeInterval(600), waitingCount: 2, requests: [request("r-1")])))
        #expect(!first.hasSameContent(as: WidgetSnapshot(host: "other", writtenAt: written, waitingCount: 2, requests: [request("r-1")])))
        #expect(!first.hasSameContent(as: WidgetSnapshot(host: "h", writtenAt: written, waitingCount: 3, requests: [request("r-1")])))
        #expect(!first.hasSameContent(as: WidgetSnapshot(host: "h", writtenAt: written, waitingCount: 2, requests: [request("r-2")])))
        #expect(!first.hasSameContent(as: WidgetSnapshot(host: "h", writtenAt: written, waitingCount: 2, requests: [request("r-1", title: "Changed")])))
    }

    @Test("the words of the widgets follow the glossary")
    func wording() {
        #expect(WidgetWording.inline(3) == "決めること 3 件")
        #expect(WidgetWording.count(12) == "12 件")
        #expect(WidgetWording.more(2) == "ほか 2 件")
        #expect(WidgetWording.updated(minutes: 180) == "3 時間前に更新")
        #expect(WidgetWording.updated(minutes: 119) == "119 分前に更新")
        #expect(WidgetWording.updated(minutes: 120) == "2 時間前に更新")
        #expect(WidgetWording.updated(minutes: 3 * 24 * 60) == "3 日前に更新")
        #expect(WidgetWording.kind(.proposal) == "稟議")
        #expect(WidgetWording.kind(.question) == "質問")
        #expect(WidgetWording.kind(.pendingDecision) == "判断待ち")
        let words = [WidgetWording.displayName, WidgetWording.description, WidgetWording.nothingWaits, WidgetWording.notSynced, WidgetWording.waitingLabel]
        for forbidden in ["ユーザー", "デバイス", "サーバー"] { #expect(!words.contains { $0.contains(forbidden) }) }
    }
}

@Suite("Widget snapshot without titles")
struct WidgetHiddenTitlesTests {
    @Test("a request without a title or a team keeps its kind, and the file has no text of it at all")
    func hidden() throws {
        let hidden = WidgetRequest(org: "acme", team: "app", request: "r-1", kind: .proposal, title: nil, teamLabel: nil)
        #expect(hidden.title == nil)
        #expect(hidden.teamLabel == nil)
        let snapshot = WidgetSnapshot(host: "h", writtenAt: written, waitingCount: 1, requests: [hidden])
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("agency-widget-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = WidgetSnapshotStore(directory: directory)
        try store.write(snapshot)
        let text = try String(contentsOf: directory.appendingPathComponent(WidgetSnapshotStore.fileName), encoding: .utf8)
        #expect(!text.contains("title"))
        #expect(!text.contains("teamLabel"))
        #expect(store.read() == snapshot)
        #expect(store.read()?.requests.first?.title == nil)
    }

    @Test("a snapshot with titles and one without are not the same content")
    func differs() {
        let shown = WidgetSnapshot(host: "h", writtenAt: written, waitingCount: 1, requests: [request()])
        let hidden = WidgetSnapshot(host: "h", writtenAt: written, waitingCount: 1, requests: [WidgetRequest(org: "acme", team: "app", request: "r-1", kind: .question, title: nil, teamLabel: nil)])
        #expect(!shown.hasSameContent(as: hidden))
    }
}

@Suite("Widget snapshot store")
struct WidgetSnapshotStoreTests {
    private func directory() throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("agency-widget-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    private func snapshot(_ count: Int = 2) -> WidgetSnapshot {
        WidgetSnapshot(host: "node.example", writtenAt: written, waitingCount: count, requests: [request("r-1", title: "承認をお願いします 🚀"), request("r-2")])
    }

    @Test("what is written is what is read, Japanese and an emoji included")
    func roundTrip() throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = WidgetSnapshotStore(directory: directory)
        try store.write(snapshot())
        #expect(store.read() == snapshot())
        #expect(store.read()?.requests[0].title == "承認をお願いします 🚀")
    }

    @Test("the file holds the count and the requests and nothing else: no token, no key, no people")
    func content() throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        try WidgetSnapshotStore(directory: directory).write(snapshot())
        let object = try JSONSerialization.jsonObject(with: Data(contentsOf: directory.appendingPathComponent(WidgetSnapshotStore.fileName))) as? [String: Any]
        #expect(Set(object?.keys.map { $0 } ?? []) == ["version", "writtenAt", "host", "waitingCount", "requests"])
        let first = (object?["requests"] as? [[String: Any]])?.first
        #expect(Set(first?.keys.map { $0 } ?? []) == ["org", "team", "request", "kind", "title", "teamLabel"])
    }

    @Test("a second write replaces the first whole")
    func overwrite() throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = WidgetSnapshotStore(directory: directory)
        try store.write(snapshot(5))
        try store.write(snapshot(1))
        #expect(store.read()?.waitingCount == 1)
        #expect(try FileManager.default.contentsOfDirectory(atPath: directory.path) == [WidgetSnapshotStore.fileName])
    }

    @Test("no file, a file that is not a snapshot, a snapshot of another version, and a file far too large are all nothing")
    func unreadable() throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = WidgetSnapshotStore(directory: directory)
        let file = directory.appendingPathComponent(WidgetSnapshotStore.fileName)
        #expect(store.read() == nil)
        try Data("not json".utf8).write(to: file)
        #expect(store.read() == nil)
        try store.write(snapshot())
        let json = try String(contentsOf: file, encoding: .utf8).replacingOccurrences(of: #""version":1"#, with: #""version":99"#)
        try Data(json.utf8).write(to: file)
        #expect(store.read() == nil)
        try Data(count: WidgetSnapshotStore.sizeLimit + 1).write(to: file)
        #expect(store.read() == nil)
        // A real snapshot that has been padded far past what one can be is not one either.
        try store.write(snapshot())
        let padded = try String(contentsOf: file, encoding: .utf8) + String(repeating: " ", count: WidgetSnapshotStore.sizeLimit)
        try Data(padded.utf8).write(to: file)
        #expect(store.read() == nil)
    }

    @Test("clearing removes the file, and clearing nothing is fine")
    func clearing() throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = WidgetSnapshotStore(directory: directory)
        store.clear()
        try store.write(snapshot())
        store.clear()
        #expect(store.read() == nil)
        #expect(!FileManager.default.fileExists(atPath: directory.appendingPathComponent(WidgetSnapshotStore.fileName).path))
    }
}

@Suite("Widget link")
struct WidgetLinkTests {
    @Test("a request's link goes there and back, and says nothing but where to go")
    func roundTrip() throws {
        let link = try #require(RequestLink(org: "acme", team: "app", request: "r-20261124-110000-a1b2"))
        let url = WidgetLink.url(for: link)
        #expect(url.absoluteString == "agency://request?org=acme&team=app&request=r-20261124-110000-a1b2")
        #expect(WidgetLink.request(from: url) == link)
    }

    @Test("any other URL is nothing: another scheme, another host, an extra or a missing value, a repeated value")
    func others() {
        let bad = [
            "https://request?org=acme&team=app&request=r-1", "agency://desk?org=acme&team=app&request=r-1",
            "agency://request?org=acme&team=app", "agency://request?org=acme&team=app&request=r-1&x=1",
            "agency://request?org=acme&org=evil&request=r-1", "agency://request", "agency:request", "agency://request?org=&team=app&request=r-1",
        ]
        for text in bad { #expect(URL(string: text).flatMap(WidgetLink.request(from:)) == nil, Comment(rawValue: text)) }
    }

    @Test("a value that is not an identifier is nothing, so nothing in a link can change which page is opened")
    func identifiers() {
        let bad = ["a%2Fb", "..%2F..", "a%20b", "r-1%0Ax", "%E3%81%82", "a%3Fb", "a%23b"]
        for value in bad { #expect(URL(string: "agency://request?org=acme&team=app&request=\(value)").flatMap(WidgetLink.request(from:)) == nil, Comment(rawValue: value)) }
        let long = String(repeating: "a", count: 129)
        #expect(URL(string: "agency://request?org=acme&team=app&request=\(long)").flatMap(WidgetLink.request(from:)) == nil)
        #expect(RequestLink(org: "a.b_c-d", team: "T1", request: "r-1") != nil)
    }
}
