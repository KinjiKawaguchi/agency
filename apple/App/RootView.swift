import SwiftUI

struct RootView: View {
    let store: HomeNodeStore
    @Bindable var push: PushCoordinator

    var body: some View {
        Group {
            if let homeNode = store.homeNode {
                HomeShell(homeNode: homeNode, push: push, onChangeHomeNode: store.clear)
                    .id(homeNode.url)
            } else {
                SetupView(onSave: store.save)
            }
        }
        .alert("Get notified?", isPresented: $push.isShowingPrompt) {
            Button("Turn on") { push.acceptPrompt() }
            Button("Not now", role: .cancel) { push.deferPrompt() }
        } message: {
            Text("Agency can notify you when a request is waiting for your decision.")
        }
    }
}
