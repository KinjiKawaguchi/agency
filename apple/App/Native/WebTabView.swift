import SwiftUI

/// A screen that is still a web page: the shared web view, or the native page that offers a way out
/// when the page cannot be loaded.
struct WebTabView: View {
    let model: NodeWebModel
    let onChangeHomeNode: () -> Void

    var body: some View {
        ZStack {
            NodeWebView(model: model).ignoresSafeArea()
            if let failure = model.failure {
                FailureView(failure: failure, onReload: model.retry, onChangeHomeNode: onChangeHomeNode)
            }
        }
        .background(Color(.systemBackground))
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
            Text(failure.title)
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
