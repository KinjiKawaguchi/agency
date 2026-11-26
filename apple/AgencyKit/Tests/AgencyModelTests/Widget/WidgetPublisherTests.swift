@testable import AgencyAPI
@testable import AgencyModel
import AgencyWidget
import Foundation
import Testing

private final class RecordingSink: WidgetFileSink {
    var written: [WidgetSnapshot] = []
    var clears = 0
    var reloads = 0
    var failsToWrite = false

    func write(_ snapshot: WidgetSnapshot) throws {
        if failsToWrite { throw CocoaError(.fileWriteUnknown) }
        written.append(snapshot)
    }

    func clear() { clears += 1 }
    func reloadWidgets() { reloads += 1 }
}

private struct MemoryStorage: WidgetPreferencesStorage {
    func load() -> WidgetPreferences { .standard }
    func save(_: WidgetPreferences) {}
}

@MainActor
@Suite("Widget publisher")
struct WidgetPublisherTests {
    private let now = Date(timeIntervalSince1970: 1_795_521_600)
    private let items = Desk.openItems([Teams.team()])

    private func publisher(_ sink: RecordingSink) -> WidgetPublisher {
        WidgetPublisher(host: "node.example", preferences: WidgetPreferencesStore(storage: MemoryStorage()), sink: sink)
    }

    @Test("the desk is written for the widgets, and the widgets are told to reload")
    func writes() {
        let sink = RecordingSink()
        publisher(sink).publish(items: items, now: now)
        #expect(sink.written.count == 1)
        #expect(sink.written.first?.waitingCount == items.filter(\.isMine).count)
        #expect(sink.reloads == 1)
    }

    @Test("a read that says the same is not written again within ten minutes, and is after that")
    func skipsSame() {
        let sink = RecordingSink()
        let publisher = publisher(sink)
        publisher.publish(items: items, now: now)
        publisher.publish(items: items, now: now.addingTimeInterval(WidgetPublisher.rewriteAfter - 1))
        #expect(sink.written.count == 1)
        publisher.publish(items: items, now: now.addingTimeInterval(WidgetPublisher.rewriteAfter))
        #expect(sink.written.count == 2)
    }

    @Test("a different desk is written at once")
    func writesChange() {
        let sink = RecordingSink()
        let publisher = publisher(sink)
        publisher.publish(items: items, now: now)
        publisher.publish(items: Array(items.dropFirst()), now: now.addingTimeInterval(1))
        #expect(sink.written.count == 2)
    }

    @Test("after clear nothing is written again: a read that was under way for the home node that was left cannot bring its desk back")
    func closedAfterClear() {
        let sink = RecordingSink()
        let publisher = publisher(sink)
        publisher.publish(items: items, now: now)
        publisher.clear()
        #expect(sink.clears == 1)
        publisher.publish(items: items, now: now.addingTimeInterval(3600))
        publisher.preferences.setShowsTitles(false)
        publisher.preferences.nodeSays(hidesTitles: true)
        #expect(sink.written.count == 1)
    }

    @Test("a change of the choice about titles writes the last desk again, without titles")
    func republishes() throws {
        let sink = RecordingSink()
        let publisher = publisher(sink)
        publisher.publish(items: items, now: now)
        publisher.preferences.setShowsTitles(false)
        #expect(sink.written.count == 2)
        let again = try #require(sink.written.last)
        #expect(again.requests.allSatisfy { $0.title == nil && $0.teamLabel == nil })
    }

    @Test("a choice made before any desk is read writes nothing")
    func nothingToRepublish() {
        let sink = RecordingSink()
        publisher(sink).preferences.setShowsTitles(false)
        #expect(sink.written.isEmpty)
    }

    @Test("a write that fails is tried again by the next read")
    func failure() {
        let sink = RecordingSink()
        sink.failsToWrite = true
        let publisher = publisher(sink)
        publisher.publish(items: items, now: now)
        #expect(sink.reloads == 0)
        sink.failsToWrite = false
        publisher.publish(items: items, now: now.addingTimeInterval(1))
        #expect(sink.written.count == 1)
    }
}
