import AgencyModel
import SwiftUI

/// The home node, and the way to change it.
struct SettingsSheet: View {
    let host: String
    let onChangeHomeNode: () -> Void
    let onResetKey: () -> Void
    let onOpenOrgChart: () -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var confirmsReset = false

    var body: some View {
        NavigationStack {
            Form {
                Section(Ja.Desk.homeNode) {
                    Text(host).textSelection(.enabled).lineLimit(2).truncationMode(.middle)
                }
                Section {
                    Button(Ja.Tabs.orgChart, action: onOpenOrgChart)
                }
                Section(footer: Text(Ja.Device.resetExplain)) {
                    Button(Ja.Device.reset, role: .destructive) { confirmsReset = true }
                }
                Section {
                    Button(Ja.Desk.changeNode, role: .destructive, action: onChangeHomeNode)
                }
            }
            .navigationTitle(Ja.Desk.settings)
            .navigationBarTitleDisplayMode(.inline)
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
}
