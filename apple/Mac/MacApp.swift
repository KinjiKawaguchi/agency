import SwiftUI

@main
struct AgencyMacApp: App {
    @NSApplicationDelegateAdaptor(MacAppDelegate.self) private var appDelegate
    @State private var store = HomeNodeStore()

    var body: some Scene {
        // One window, so there is one session: a second window would be a second session reading and signing.
        Window("Agency", id: "main") {
            RootView(store: store, push: appDelegate.push)
                .frame(minWidth: 820, minHeight: 560)
        }
    }
}
