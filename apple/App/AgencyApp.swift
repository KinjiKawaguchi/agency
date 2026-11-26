import AgencyWidget
import SwiftUI

@main
struct AgencyApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @State private var store = HomeNodeStore()

    var body: some Scene {
        WindowGroup {
            RootView(store: store, push: appDelegate.push)
                // A widget's tap: it opens the request it names, as a tapped notification does.
                .onOpenURL { url in
                    if let link = WidgetLink.request(from: url) { appDelegate.push.open(.request(link)) }
                }
        }
    }
}
