import Foundation
import Observation

/// What the owner chose about the widgets, for one home node.
public struct WidgetPreferences: Equatable, Sendable {
    /// Whether a widget shows the titles and the teams of the requests. Off, it shows only their kinds and the count.
    public var showsTitles: Bool

    public static let standard = WidgetPreferences(showsTitles: true)

    public init(showsTitles: Bool) {
        self.showsTitles = showsTitles
    }
}

/// Where the choice is kept between launches.
public protocol WidgetPreferencesStorage: Sendable {
    func load() -> WidgetPreferences
    func save(_ preferences: WidgetPreferences)
}

/// The owner's choice about the widgets, and what the node says about titles. The choice can only hide more: a node
/// that keeps titles off its devices keeps them off the widgets, whatever the choice says.
@MainActor
@Observable
public final class WidgetPreferencesStore {
    public private(set) var preferences: WidgetPreferences
    /// Whether the node hides titles (`Me.hidesTitles`), or `nil` until it has said: the owner's switch cannot show what the
    /// node hides, and nothing is shown on a guess.
    public private(set) var nodeHidesTitles: Bool?

    @ObservationIgnored private let storage: any WidgetPreferencesStorage
    /// Called after the choice or the node's word changes, so the widgets are written again.
    @ObservationIgnored public var onChange: @MainActor () -> Void = {}

    public init(storage: any WidgetPreferencesStorage) {
        self.storage = storage
        preferences = storage.load()
    }

    /// Whether the widgets show titles now: the owner wants them and the node has said it does not hide them. Until the node
    /// has answered, none are shown.
    public var showsTitles: Bool { preferences.showsTitles && nodeHidesTitles == false }

    /// How the switch in the settings reads: off when the node hides titles, the owner's choice otherwise.
    public var isSwitchOn: Bool { nodeHidesTitles == true ? false : preferences.showsTitles }

    public func setShowsTitles(_ shows: Bool) {
        guard shows != preferences.showsTitles else { return }
        preferences.showsTitles = shows
        storage.save(preferences)
        onChange()
    }

    /// What the node says about titles, learned from the account it answers with.
    public func nodeSays(hidesTitles: Bool) {
        guard hidesTitles != nodeHidesTitles else { return }
        nodeHidesTitles = hidesTitles
        onChange()
    }
}
