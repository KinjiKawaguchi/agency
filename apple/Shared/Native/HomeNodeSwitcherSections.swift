import AgencyKit
import AgencyModel
import SwiftUI

/// The current home node and the others this device knows, with the way to go to one, add one, and forget one.
/// Switching keeps the sign-in, the key and the choices of the node left; forgetting removes them, after the node has been told
/// to stop its notifications. Only a node the owner added is forgotten here: one the current node names comes back from it.
struct HomeNodeSwitcherSections: View {
    let homeNode: HomeNodeURL
    let nodes: any HomeNodeSwitching
    let onSwitch: (HomeNodeURL) -> Void
    let onStopPush: (HomeNodeURL) async -> Bool
    @State private var isAdding = false
    @State private var forgetting: HomeNodeEntry?
    @State private var cannotStopPush: HomeNodeEntry?
    @State private var isStoppingPush = false

    var body: some View {
        Section(Ja.HomeNodes.current) {
            Label { Text(homeNode.host).textSelection(.enabled).lineLimit(2).truncationMode(.middle) } icon: { Image(systemName: "checkmark.circle.fill") }
        }
        Section {
            if nodes.others.isEmpty {
                Text(Ja.HomeNodes.none).foregroundStyle(.secondary)
            }
            ForEach(nodes.others, id: \.url.url) { entry in
                Button { onSwitch(entry.url) } label: { row(entry) }
                    .disabled(isStoppingPush)
                    .swipeActions { if entry.source == .added { forgetButton(entry) } }
                    .contextMenu { if entry.source == .added { forgetButton(entry) } }
            }
            Button(Ja.HomeNodes.add) { isAdding = true }
            if isStoppingPush {
                HStack { ProgressView(); Text(Ja.HomeNodes.working).foregroundStyle(.secondary) }
            }
        } header: {
            Text(Ja.HomeNodes.others)
        } footer: {
            Text(Ja.HomeNodes.switchHint)
        }
        .sheet(isPresented: $isAdding) {
            AddHomeNodeSheet { node, label in nodes.add(node, label: label) }
        }
        .confirmationDialog(Ja.HomeNodes.forget, isPresented: Binding(get: { forgetting != nil }, set: { if !$0 { forgetting = nil } }), titleVisibility: .visible, presenting: forgetting) { entry in
            Button(Ja.HomeNodes.forgetConfirm, role: .destructive) { forget(entry) }
        } message: { entry in
            Text(Ja.HomeNodes.forgetExplain(entry.url.host))
        }
        .alert(Ja.HomeNodes.stopFailedTitle, isPresented: Binding(get: { cannotStopPush != nil }, set: { if !$0 { cannotStopPush = nil } }), presenting: cannotStopPush) { entry in
            Button(Ja.HomeNodes.forgetAnyway, role: .destructive) { nodes.forget(entry.url) }
            Button(Ja.HomeNodes.cancel, role: .cancel) {}
        } message: { entry in
            Text(Ja.HomeNodes.stopFailed(entry.url.host))
        }
    }

    private func forgetButton(_ entry: HomeNodeEntry) -> some View {
        Button(Ja.HomeNodes.forget, role: .destructive) { forgetting = entry }
    }

    /// The node is told first, while its sign-in is still here; when it cannot be told, the owner decides.
    private func forget(_ entry: HomeNodeEntry) {
        isStoppingPush = true
        Task {
            let isStopped = await onStopPush(entry.url)
            isStoppingPush = false
            if isStopped { nodes.forget(entry.url) } else { cannotStopPush = entry }
        }
    }

    private func row(_ entry: HomeNodeEntry) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(entry.label).foregroundStyle(.primary).fixedSize(horizontal: false, vertical: true)
            Text(entry.url.host).font(.footnote).foregroundStyle(.secondary).lineLimit(2).truncationMode(.middle)
            if entry.source == .configured {
                Text(Ja.HomeNodes.configured).font(.caption).foregroundStyle(.secondary)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .contentShape(Rectangle())
    }
}

/// Asks for the address of a home node (and a name for it) to put on the list.
struct AddHomeNodeSheet: View {
    let onAdd: (HomeNodeURL, String) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var address = ""
    @State private var label = ""
    @State private var problem: String?

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField(Ja.HomeNodes.addressField, text: $address)
                        #if os(iOS)
                        .keyboardType(.URL)
                        .textInputAutocapitalization(.never)
                        #endif
                        .autocorrectionDisabled()
                    TextField(Ja.HomeNodes.labelField, text: $label)
                } footer: {
                    if let problem { Text(problem).foregroundStyle(.red) }
                }
            }
            .navigationTitle(Ja.HomeNodes.add)
            #if os(iOS)
            .navigationBarTitleDisplayMode(.inline)
            #endif
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button(Ja.Desk.close) { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button(Ja.HomeNodes.addConfirm, action: add).disabled(address.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
            }
        }
        .presentationDetents([.medium])
    }

    private func add() {
        do {
            onAdd(try HomeNodeURL(validating: address), label)
            dismiss()
        } catch let error as HomeNodeURLError {
            problem = error.message
        } catch {
            problem = HomeNodeURLError.invalid.message
        }
    }
}
