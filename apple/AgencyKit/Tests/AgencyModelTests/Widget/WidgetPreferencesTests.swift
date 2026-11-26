@testable import AgencyModel
import Foundation
import Testing

private final class MemoryStorage: WidgetPreferencesStorage, @unchecked Sendable {
    private let lock = NSLock()
    private var stored: WidgetPreferences?
    var saves = 0

    init(_ stored: WidgetPreferences? = nil) { self.stored = stored }

    func load() -> WidgetPreferences { lock.withLock { stored ?? .standard } }
    func save(_ preferences: WidgetPreferences) { lock.withLock { stored = preferences; saves += 1 } }
}

@MainActor
@Suite("Widget preferences")
struct WidgetPreferencesTests {
    @Test("titles are shown until the owner turns them off, and the choice is kept")
    func choice() {
        let storage = MemoryStorage()
        let store = WidgetPreferencesStore(storage: storage)
        #expect(store.showsTitles)
        store.setShowsTitles(false)
        #expect(!store.showsTitles)
        #expect(WidgetPreferencesStore(storage: storage).preferences.showsTitles == false)
    }

    @Test("a node that hides titles hides them on the widgets, whatever the choice says; the choice can only hide more")
    func nodeWins() {
        let store = WidgetPreferencesStore(storage: MemoryStorage())
        #expect(store.preferences.showsTitles)
        store.nodeSays(hidesTitles: true)
        #expect(store.preferences.showsTitles)
        #expect(!store.showsTitles)
        store.nodeSays(hidesTitles: false)
        #expect(store.showsTitles)
        store.setShowsTitles(false)
        store.nodeSays(hidesTitles: false)
        #expect(!store.showsTitles)
    }

    @Test("the widgets are written again when the choice or the node's word changes, and not when nothing changed")
    func writesAgain() {
        var changes = 0
        let storage = MemoryStorage()
        let store = WidgetPreferencesStore(storage: storage)
        store.onChange = { changes += 1 }
        store.setShowsTitles(true)
        #expect(changes == 0)
        #expect(storage.saves == 0)
        store.setShowsTitles(false)
        #expect(changes == 1)
        store.nodeSays(hidesTitles: true)
        #expect(changes == 2)
        store.nodeSays(hidesTitles: true)
        #expect(changes == 2)
    }
}
