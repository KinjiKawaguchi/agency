import Foundation
import Observation

/// Where the preferences of one home node are kept between launches.
public protocol NotificationPreferencesStorage: Sendable {
    func load() -> NotificationPreferences?
    func save(_ preferences: NotificationPreferences)
}

/// The owner's choices of what to be told, for one home node. A change is kept at once and handed to
/// `onChange`, which is how the banners and the push registration follow it.
@MainActor
@Observable
public final class NotificationPreferencesStore {
    public private(set) var preferences: NotificationPreferences

    @ObservationIgnored private let storage: any NotificationPreferencesStorage
    @ObservationIgnored private let onChange: @MainActor (NotificationPreferences) -> Void

    public init(storage: any NotificationPreferencesStorage, onChange: @escaping @MainActor (NotificationPreferences) -> Void = { _ in }) {
        self.storage = storage
        self.onChange = onChange
        preferences = storage.load() ?? .standard
    }

    public func setBanner(_ kind: EventKind, isOn: Bool) {
        var next = preferences
        if isOn { next.bannerKinds.insert(kind) } else { next.bannerKinds.remove(kind) }
        update(next)
    }

    public func setPush(_ kind: PushKind, isOn: Bool) {
        var next = preferences
        if isOn { next.pushKinds.insert(kind) } else { next.pushKinds.remove(kind) }
        update(next)
    }

    private func update(_ next: NotificationPreferences) {
        guard next != preferences else { return }
        preferences = next
        storage.save(next)
        onChange(next)
    }
}
