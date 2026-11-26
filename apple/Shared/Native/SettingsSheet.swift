import AgencyKit
import AgencyModel
import AgencyUI
import AgencyWidget
import SwiftUI

/// The home nodes (switch, add, forget), what the app tells the owner, and the way to leave this one.
struct SettingsSheet: View {
    let homeNode: HomeNodeURL
    let nodes: any HomeNodeSwitching
    let notifications: NotificationPreferencesStore
    let widgets: WidgetPreferencesStore
    let nodeSendsPushKinds: Bool?
    let onSwitchHomeNode: (HomeNodeURL) -> Void
    let onStopPush: (HomeNodeURL) async -> Bool
    let onChangeHomeNode: () -> Void
    let onResetKey: () -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var confirmsReset = false
    @State private var isStoppingPush = false
    @State private var cannotStopPush = false

    var body: some View {
        NavigationStack {
            Form {
                HomeNodeSwitcherSections(homeNode: homeNode, nodes: nodes, onSwitch: onSwitchHomeNode, onStopPush: onStopPush)
                NotificationSettingsSections(store: notifications, nodeSendsKinds: nodeSendsPushKinds)
                widgetSection
                Section(footer: Text(Ja.Key.resetExplain(.platform))) {
                    Button(Ja.Device.reset, role: .destructive) { confirmsReset = true }
                }
                Section {
                    Button(Ja.Desk.changeNode, role: .destructive, action: leave).disabled(isStoppingPush)
                }
            }
            .navigationTitle(Ja.Desk.settings)
            #if os(iOS)
            .navigationBarTitleDisplayMode(.inline)
            #endif
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button(Ja.Desk.close) { dismiss() } }
            }
        }
        .confirmationDialog(Ja.Device.reset, isPresented: $confirmsReset, titleVisibility: .visible) {
            Button(Ja.Device.resetConfirm, role: .destructive, action: onResetKey)
        } message: {
            Text(Ja.Key.resetExplain(.platform))
        }
        .alert(Ja.HomeNodes.stopFailedTitle, isPresented: $cannotStopPush) {
            Button(Ja.HomeNodes.leaveAnyway, role: .destructive, action: onChangeHomeNode)
            Button(Ja.HomeNodes.cancel, role: .cancel) {}
        } message: {
            Text(Ja.HomeNodes.stopFailed(homeNode.host))
        }
        .presentationDetents([.medium, .large])
    }

    /// Leaving tells the node to stop its notifications first, while the sign-in still works; when it cannot be told, the
    /// owner decides.
    private func leave() {
        isStoppingPush = true
        Task {
            let isStopped = await onStopPush(homeNode)
            isStoppingPush = false
            if isStopped { onChangeHomeNode() } else { cannotStopPush = true }
        }
    }

    /// Whether the widgets show titles. A node that keeps titles off devices keeps them off the widgets, and the row says so.
    private var widgetSection: some View {
        Section(footer: Text(widgets.nodeHidesTitles == true ? WidgetWording.nodeHidesTitles : WidgetWording.showTitlesFooter)) {
            Toggle(WidgetWording.showTitles, isOn: Binding(get: { widgets.isSwitchOn }, set: { widgets.setShowsTitles($0) }))
                .disabled(widgets.nodeHidesTitles == true)
        }
    }
}
