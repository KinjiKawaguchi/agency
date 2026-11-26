import AgencyModel
import AppKit
import SwiftUI

/// The menu-bar item's label: the symbol, and how many requests wait when some do.
struct MenuBarLabel: View {
    let status: WaitingStatus

    var body: some View {
        if let text = Ja.MenuBar.label(status.count) {
            Label(text, systemImage: "tray.full")
        } else {
            Image(systemName: "tray")
        }
    }
}

/// The menu-bar item's menu: how many requests wait, and the way to the window.
struct MenuBarContent: View {
    let status: WaitingStatus
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        Text(Ja.MenuBar.status(status.count))
        Divider()
        Button(Ja.MenuBar.open, action: open)
        Button(Ja.MenuBar.quit) { NSApplication.shared.terminate(nil) }
            .keyboardShortcut("q")
    }

    private func open() {
        NSApplication.shared.activate(ignoringOtherApps: true)
        if let window = NSApplication.shared.windows.first(where: \.canBecomeMain) {
            window.makeKeyAndOrderFront(nil)
        } else {
            openWindow(id: MacApp.windowID)
        }
    }
}
