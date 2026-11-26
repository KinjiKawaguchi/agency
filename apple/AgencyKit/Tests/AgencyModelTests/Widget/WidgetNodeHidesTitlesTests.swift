@testable import AgencyAPI
@testable import AgencyModel
@testable import AgencyWidget
import Foundation
import Testing

/// What a node that hides titles (`hidesTitles` in its answer to `GET /api/me`) must get from the widgets: the file the
/// widgets read, which is all they have, holds no title and no team label. The answers are the ones the node records.
@MainActor
@Suite("Widgets and a node that hides titles")
struct WidgetNodeHidesTitlesTests {
    private let now = Date(timeIntervalSince1970: 1_795_521_600)

    private struct FileSink: WidgetFileSink {
        let store: WidgetSnapshotStore

        func write(_ snapshot: WidgetSnapshot) throws { try store.write(snapshot) }
        func clear() { store.clear() }
        func reloadWidgets() {}
    }

    private struct MemoryStorage: WidgetPreferencesStorage {
        let showsTitles: Bool

        func load() -> WidgetPreferences { WidgetPreferences(showsTitles: showsTitles) }
        func save(_: WidgetPreferences) {}
    }

    private func recordedAnswer(_ name: String, from file: String = #filePath) throws -> Me {
        // Tests/AgencyModelTests/Widget/<file>: six levels up is the repository root.
        var url = URL(fileURLWithPath: file)
        for _ in 0..<6 { url.deleteLastPathComponent() }
        let data = try Data(contentsOf: url.appendingPathComponent("testdata/client/\(name).json"))
        return try JSONDecoder().decode(Me.self, from: data)
    }

    /// The file the widgets would read after the app learned `answer` from the node, with the owner's switch as given.
    private func fileText(after answer: Me, ownerShowsTitles: Bool) throws -> (text: String, snapshot: WidgetSnapshot) {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("agency-widget-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = WidgetSnapshotStore(directory: directory)
        let preferences = WidgetPreferencesStore(storage: MemoryStorage(showsTitles: ownerShowsTitles))
        let publisher = WidgetPublisher(host: "node.example", preferences: preferences, sink: FileSink(store: store))
        // The order of the app: the node's answer first, then the desk.
        preferences.nodeSays(hidesTitles: answer.hidesTitles)
        publisher.publish(items: Desk.openItems([Teams.team()]), now: now)
        let snapshot = try #require(store.read())
        return (try String(contentsOf: directory.appendingPathComponent(WidgetSnapshotStore.fileName), encoding: .utf8), snapshot)
    }

    @Test("a node that hides titles gets a file with the count and the kinds, and no title or team label, whatever the owner chose")
    func hidden() throws {
        let answer = try recordedAnswer("me-hides-titles")
        let items = Desk.openItems([Teams.team()])
        for ownerShowsTitles in [true, false] {
            let (text, snapshot) = try fileText(after: answer, ownerShowsTitles: ownerShowsTitles)
            #expect(snapshot.waitingCount == items.filter(\.isMine).count)
            #expect(!snapshot.requests.isEmpty)
            #expect(snapshot.requests.allSatisfy { $0.title == nil && $0.teamLabel == nil })
            for item in items {
                #expect(!text.contains(item.request.title), "a title is in the file")
                #expect(!text.contains(item.teamLabel), "a team label is in the file")
            }
        }
    }

    @Test("a node that does not hide titles gets them in the file, unless the owner turned them off")
    func shown() throws {
        let answer = try recordedAnswer("me")
        let items = Desk.openItems([Teams.team()])
        let shown = try fileText(after: answer, ownerShowsTitles: true)
        #expect(shown.snapshot.requests.allSatisfy { $0.title != nil && $0.teamLabel != nil })
        #expect(shown.text.contains(try #require(shown.snapshot.requests.first?.title)))
        let off = try fileText(after: answer, ownerShowsTitles: false)
        #expect(off.snapshot.requests.allSatisfy { $0.title == nil && $0.teamLabel == nil })
        #expect(items.contains { !off.text.contains($0.request.title) })
    }
}
