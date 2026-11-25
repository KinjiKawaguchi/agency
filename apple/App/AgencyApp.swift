import SwiftUI

@main
struct AgencyApp: App {
    @State private var store = HomeNodeStore()

    var body: some Scene {
        WindowGroup {
            RootView(store: store)
        }
    }
}
