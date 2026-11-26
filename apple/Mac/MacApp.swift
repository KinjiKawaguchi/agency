import SwiftUI

@main
struct AgencyMacApp: App {
    @NSApplicationDelegateAdaptor(MacAppDelegate.self) private var appDelegate
    @State private var store = HomeNodeStore()

    var body: some Scene {
        WindowGroup {
            RootView(store: store, push: appDelegate.push)
                .frame(minWidth: 820, minHeight: 560)
        }
    }
}
