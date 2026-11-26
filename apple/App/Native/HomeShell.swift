import AgencyKit
import AgencyModel
import SwiftUI

/// The app for one home node on the iPhone: a tab for each screen.
struct HomeShell: View {
    @State private var model: ShellModel

    init(homeNode: HomeNodeURL, push: PushCoordinator, onChangeHomeNode: @escaping () -> Void) {
        _model = State(initialValue: ShellModel(homeNode: homeNode, push: push, deviceLabel: Ja.Device.label, onChangeHomeNode: onChangeHomeNode))
    }

    var body: some View {
        Group {
            if let session = model.session {
                TabView(selection: $model.tab) {
                    ForEach(AppTab.allCases, id: \.self) { tab in
                        ShellScreen(tab: tab, model: model, session: session)
                            .tabItem { Label(tab.title, systemImage: tab.symbol) }
                            .tag(tab)
                    }
                }
            } else {
                ProgressView()
            }
        }
        .shellChrome(model)
    }
}
