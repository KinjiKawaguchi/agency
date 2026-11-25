import AgencyKit
import SwiftUI

/// The home node in a web view, or the native page that offers a way out when it cannot be loaded.
struct NodeScreen: View {
    let homeNode: HomeNodeURL
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
                        onChangeHomeNode: onChangeHomeNode
                    )
                }
            }
        }
        .background(Color(.systemBackground))
        .task {
            if model == nil { model = NodeWebModel(homeNode: homeNode) }
        }
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
