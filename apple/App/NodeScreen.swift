import AgencyKit
import SwiftUI

/// The home node in a web view, or the native page that offers a way out when it cannot be loaded.
struct NodeScreen: View {
    let homeNode: HomeNodeURL
    let push: PushCoordinator
    let onChangeHomeNode: () -> Void

    @State private var model: NodeWebModel?

    var body: some View {
        ZStack {
            if let model {
                NodeWebView(model: model)
                    .ignoresSafeArea()
                if let failure = model.failure {
                    FailureView(
                        failure: failure,
                        onReload: model.retry,
                        onChangeHomeNode: changeHomeNode
                    )
                }
            }
        }
        .background(Color(.systemBackground))
        .task { startModel() }
        .onChange(of: push.registration) { _, registration in
            model?.updatePush(registration)
        }
        .onChange(of: push.pendingLink) {
            if let model, let link = push.takePendingLink() { model.open(link) }
        }
    }
}

extension NodeScreen {
    /// The number on the icon belongs to the node that is being left.
    private func changeHomeNode() {
        push.setBadge(0)
        onChangeHomeNode()
    }

    private func startModel() {
        guard model == nil else { return }
        let model = NodeWebModel(
            homeNode: homeNode,
            push: push.registration,
            startingAt: push.takePendingLink()
        )
        model.onHomeNodeReached = { push.noteSignedIn() }
        model.onBadgeChange = { push.setBadge($0) }
        self.model = model
    }
}

private struct FailureView: View {
    let failure: LoadFailure
    let onReload: () -> Void
    let onChangeHomeNode: () -> Void

    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "wifi.exclamationmark")
                .font(.system(size: 44))
                .foregroundStyle(.secondary)
            Text("Cannot reach your home node")
                .font(.title3.bold())
            Text(failure.message)
                .font(.footnote)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
            Button("Reload", action: onReload)
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
            Button("Change home node", action: onChangeHomeNode)
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(.systemBackground))
    }
}
