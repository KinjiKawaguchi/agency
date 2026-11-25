import SwiftUI

struct RootView: View {
    let store: HomeNodeStore

    var body: some View {
        if let homeNode = store.homeNode {
            NodeScreen(homeNode: homeNode, onChangeHomeNode: store.clear)
                .id(homeNode.url)
        } else {
            SetupView(onSave: store.save)
        }
    }
}
