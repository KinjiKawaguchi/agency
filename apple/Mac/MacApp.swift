import SwiftUI

@main
struct MacApp: App {
    static let windowID = "main"

    @NSApplicationDelegateAdaptor(MacAppDelegate.self) private var appDelegate
    @State private var store = HomeNodeStore()
    @State private var waiting = WaitingStatus()

    var body: some Scene {
        // One window, so there is one session: a second window would be a second session reading and signing.
        Window("Agency", id: Self.windowID) {
            RootView(store: store, push: appDelegate.push)
                .frame(minWidth: 820, minHeight: 560)
                .environment(\.waitingStatus, waiting)
        }
        .commands { SettingsCommand() }
        MenuBarExtra {
            MenuBarContent(status: waiting)
        } label: {
            MenuBarLabel(status: waiting)
        }
    }
}
