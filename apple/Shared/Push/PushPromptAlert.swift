import SwiftUI

extension View {
    /// The one-line explanation before the system asks for permission to notify (see `PushPromptPolicy`).
    func pushPrompt(_ push: PushCoordinator) -> some View {
        modifier(PushPromptAlert(push: push))
    }
}

private struct PushPromptAlert: ViewModifier {
    @Bindable var push: PushCoordinator

    func body(content: Content) -> some View {
        content.alert("Get notified?", isPresented: $push.isShowingPrompt) {
            Button("Turn on") { push.acceptPrompt() }
            Button("Not now", role: .cancel) { push.deferPrompt() }
        } message: {
            Text("Agency can notify you when a request is waiting for your decision.")
        }
    }
}
