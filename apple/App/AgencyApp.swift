import SwiftUI

@main
struct AgencyApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @State private var store = HomeNodeStore()

    var body: some Scene {
        WindowGroup {
            RootView(store: store, push: appDelegate.push)
        }
    }
}
