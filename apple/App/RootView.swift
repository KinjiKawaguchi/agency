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
        #if os(iOS)
        .task { await LegacyWebData.removeOnce() }
        #endif
        .pushPrompt(push)
    }
}
