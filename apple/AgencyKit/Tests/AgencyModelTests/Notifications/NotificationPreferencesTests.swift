@testable import AgencyModel
import Foundation
import Testing

private final class MemoryStorage: NotificationPreferencesStorage, @unchecked Sendable {
    private let lock = NSLock()
    private var kept: NotificationPreferences?
    private var saves = 0

    init(_ kept: NotificationPreferences? = nil) { self.kept = kept }
    func load() -> NotificationPreferences? { lock.withLock { kept } }
    func save(_ preferences: NotificationPreferences) { lock.withLock { kept = preferences; saves += 1 } }
    var saveCount: Int { lock.withLock { saves } }
}

@Suite("NotificationPreferences")
struct NotificationPreferencesTests {
    @Test("every kind starts on, and the push kinds are named as the node names them, in a fixed order")
    func standard() {
        #expect(NotificationPreferences.standard.bannerKinds == Set(EventKind.allCases))
        #expect(NotificationPreferences.standard.pushEventNames == ["request", "device", "decision"])
        #expect(NotificationPreferences(bannerKinds: [], pushKinds: [.decision, .request]).pushEventNames == ["request", "decision"])
        #expect(NotificationPreferences(bannerKinds: [], pushKinds: []).pushEventNames == [])
    }

    @Test("a push kind is the push for the kind of news of the same name")
    func eventKinds() {
        #expect(PushKind.allCases.map(\.eventKind.name) == PushKind.allCases.map(\.rawValue))
    }

    @Test("it is written as names, and what is read back is the same")
    func roundTrip() throws {
        let value = NotificationPreferences(bannerKinds: [.request, .blocked], pushKinds: [.device])
        let data = try JSONEncoder().encode(value)
        #expect(String(decoding: data, as: UTF8.self).contains(#""banners":["request","blocked"]"#))
        #expect(String(decoding: data, as: UTF8.self).contains(#""pushes":["device"]"#))
        #expect(try JSONDecoder().decode(NotificationPreferences.self, from: data) == value)
    }

    @Test("a name that is not known is dropped, and the others are kept")
    func unknownNames() throws {
        let text = #"{"banners":["request","teleport"],"pushes":["decision","fax"]}"#
        let value = try JSONDecoder().decode(NotificationPreferences.self, from: Data(text.utf8))
        #expect(value == NotificationPreferences(bannerKinds: [.request], pushKinds: [.decision]))
    }

    @Test("a stored value that is not readable is an error, not 'everything off'")
    func unreadable() {
        #expect(throws: (any Error).self) { try JSONDecoder().decode(NotificationPreferences.self, from: Data(#"{"banners":[]}"#.utf8)) }
    }
}

@MainActor
@Suite("NotificationPreferencesStore")
struct NotificationPreferencesStoreTests {
    @Test("it starts from what was kept, or from every kind on")
    func start() {
        #expect(NotificationPreferencesStore(storage: MemoryStorage()).preferences == .standard)
        let kept = NotificationPreferences(bannerKinds: [.device], pushKinds: [])
        #expect(NotificationPreferencesStore(storage: MemoryStorage(kept)).preferences == kept)
    }

    @Test("turning a banner or a push off or on is kept and told once; the same choice again is not")
    func changes() {
        let storage = MemoryStorage()
        var told: [NotificationPreferences] = []
        let store = NotificationPreferencesStore(storage: storage) { told.append($0) }
        store.setBanner(.blocked, isOn: false)
        store.setBanner(.blocked, isOn: false)
        store.setPush(.device, isOn: false)
        #expect(store.preferences.bannerKinds == Set(EventKind.allCases).subtracting([.blocked]))
        #expect(store.preferences.pushKinds == [.request, .decision])
        #expect(storage.load() == store.preferences)
        #expect(storage.saveCount == 2)
        #expect(told.count == 2 && told.last == store.preferences)
        store.setBanner(.blocked, isOn: true)
        store.setPush(.device, isOn: true)
        #expect(store.preferences == .standard)
        #expect(told.count == 4)
    }
}
