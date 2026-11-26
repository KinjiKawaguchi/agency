import SwiftUI

@main
struct MacApp: App {
    static let windowID = "main"

    @NSApplicationDelegateAdaptor(MacAppDelegate.self) private var appDelegate

    var body: some Scene {
        // One window, so there is one view of the session; the session itself belongs to the app, not to the window.
        Window("Agency", id: Self.windowID) {
            MacRootView(model: appDelegate.model, push: appDelegate.push)
                .frame(minWidth: 820, minHeight: 560)
        }
        .commands { SettingsCommand() }
        MenuBarExtra {
            MenuBarContent(model: appDelegate.model)
        } label: {
            MenuBarLabel(model: appDelegate.model)
        }
    }
}
