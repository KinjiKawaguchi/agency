import AgencyModel
import AgencyUI
import AgencyWidget
import SwiftUI

/// The home node, what the app tells the owner, and the way to change the home node.
struct SettingsSheet: View {
    let host: String
    let notifications: NotificationPreferencesStore
    let widgets: WidgetPreferencesStore
    let nodeSendsPushKinds: Bool
    let onChangeHomeNode: () -> Void
    let onResetKey: () -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var confirmsReset = false

    var body: some View {
        NavigationStack {
            Form {
                Section(Ja.Desk.homeNode) {
                    Text(host).textSelection(.enabled).lineLimit(2).truncationMode(.middle)
                }
                NotificationSettingsSections(store: notifications, nodeSendsKinds: nodeSendsPushKinds)
                widgetSection
                Section(footer: Text(Ja.Device.resetExplain)) {
                    Button(Ja.Device.reset, role: .destructive) { confirmsReset = true }
                }
                Section {
                    Button(Ja.Desk.changeNode, role: .destructive, action: onChangeHomeNode)
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
            Text(Ja.Device.resetExplain)
        }
        .presentationDetents([.medium, .large])
    }

    /// Whether the widgets show titles. A node that keeps titles off devices keeps them off the widgets, and the row says so.
    private var widgetSection: some View {
        Section(footer: Text(widgets.nodeHidesTitles ? WidgetWording.nodeHidesTitles : WidgetWording.showTitlesFooter)) {
            Toggle(WidgetWording.showTitles, isOn: Binding(get: { widgets.showsTitles }, set: { widgets.setShowsTitles($0) }))
                .disabled(widgets.nodeHidesTitles)
        }
    }
}
