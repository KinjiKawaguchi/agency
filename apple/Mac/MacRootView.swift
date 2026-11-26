import SwiftUI

/// The Mac's window: the shell the app holds, or the setup when there is no home node.
struct MacRootView: View {
    let model: MacAppModel
    @Bindable var push: PushCoordinator

    var body: some View {
        Group {
            if let shell = model.shell {
                HomeShell(model: shell)
            } else {
                SetupView(onSave: model.save)
            }
        }
        .pushPrompt(push)
    }
}
