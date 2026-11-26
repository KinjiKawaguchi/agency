import AgencyModel
import SwiftUI

/// The app for one home node on the Mac: a sidebar with the screens, and the open screen beside it.
struct HomeShell: View {
    let model: ShellModel

    var body: some View {
        NavigationSplitView {
            List(AppTab.allCases, id: \.self, selection: Bindable(model.router).tab) { tab in
                Label(tab.title, systemImage: tab.symbol)
            }
            .navigationSplitViewColumnWidth(min: 160, ideal: 190, max: 260)
            .toolbar {
                ToolbarItem {
                    Button(Ja.Desk.settings, systemImage: "gearshape") { model.showsSettings = true }
                }
            }
        } detail: {
            if let session = model.session {
                ShellScreen(tab: model.router.tab, model: model, session: session)
            } else {
                ProgressView()
            }
        }
        .shellChrome(model)
        .focusedSceneValue(\.shellModel, model)
    }
}
