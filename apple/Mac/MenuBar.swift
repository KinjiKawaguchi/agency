import AgencyModel
import AppKit
import SwiftUI

/// The menu-bar item's label: the symbol, and how many requests wait when some do.
struct MenuBarLabel: View {
    let model: MacAppModel

    var body: some View {
        Group {
            if let text = Ja.MenuBar.label(model.waitingCount) {
                Label(text, systemImage: "tray.full")
            } else {
                Image(systemName: "tray")
            }
        }
        .accessibilityLabel(Ja.MenuBar.accessibility(model.waitingCount))
    }
}

/// The menu-bar item's menu: how many requests wait, and the way to the window.
struct MenuBarContent: View {
    let model: MacAppModel
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        Text(Ja.MenuBar.status(model.waitingCount))
        Divider()
        Button(Ja.MenuBar.open, action: open)
        Button(Ja.MenuBar.quit) { NSApplication.shared.terminate(nil) }
            .keyboardShortcut("q")
    }

    private func open() {
        NSApplication.shared.activate(ignoringOtherApps: true)
        if let window = NSApplication.shared.windows.first(where: \.canBecomeMain) {
            if window.isMiniaturized { window.deminiaturize(nil) }
            window.makeKeyAndOrderFront(nil)
        } else {
            openWindow(id: MacApp.windowID)
        }
    }
}
