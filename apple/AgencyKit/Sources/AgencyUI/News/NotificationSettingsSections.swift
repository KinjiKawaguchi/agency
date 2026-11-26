#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// What the app tells the owner, as two sections for a form: while the app is open, and while it is closed.
/// A switch for each kind of news; the text wraps, so nothing here is wider than the screen.
public struct NotificationSettingsSections: View {
    private let store: NotificationPreferencesStore
    private let nodeSendsKinds: Bool?

    /// - Parameter nodeSendsKinds: `false` when the home node is older than kinds and sends requests only; the
    ///   other kinds cannot be turned on then, and the screen says why. `nil` while the node has not said: the screen
    ///   claims nothing about the node.
    public init(store: NotificationPreferencesStore, nodeSendsKinds: Bool? = nil) {
        self.store = store
        self.nodeSendsKinds = nodeSendsKinds
    }

    public var body: some View {
        Section {
            ForEach(EventKind.allCases, id: \.self) { kind in
                Toggle(
                    Ja.NotificationSettings.title(of: kind),
                    isOn: Binding(get: { store.preferences.bannerKinds.contains(kind) }, set: { store.setBanner(kind, isOn: $0) })
                )
            }
        } header: {
            Text(Ja.NotificationSettings.appOpenHeading)
        } footer: {
            Text(Ja.NotificationSettings.appOpenFooter)
        }
        Section {
            ForEach(PushKind.allCases, id: \.self) { kind in
                Toggle(
                    Ja.NotificationSettings.title(of: kind),
                    isOn: Binding(get: { store.preferences.pushKinds.contains(kind) }, set: { store.setPush(kind, isOn: $0) })
                )
                .disabled(kind != .request && nodeSendsKinds == false)
            }
        } header: {
            Text(Ja.NotificationSettings.appClosedHeading)
        } footer: {
            Text(nodeSendsKinds == false ? Ja.NotificationSettings.requestsOnly + Ja.NotificationSettings.appClosedFooter : Ja.NotificationSettings.appClosedFooter)
        }
    }
}
#endif
