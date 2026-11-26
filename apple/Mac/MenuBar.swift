import AgencyModel
import AppKit
import SwiftUI

/// The menu-bar item's label: the symbol, and how many requests wait when some do.
struct MenuBarLabel: View {
    let model: MacAppModel

    var body: some View {
        let state = model.menuState
        let recoveries = model.recoveryLines
        let symbol = Ja.MenuBar.symbol(state, hasRecovery: !recoveries.isEmpty)
        Group {
            if let text = Ja.MenuBar.label(state) {
                Label(text, systemImage: symbol)
            } else {
                Image(systemName: symbol)
            }
        }
        .opacity(Ja.MenuBar.isDim(state) ? 0.5 : 1)
        .accessibilityLabel(Ja.MenuBar.accessibility(state, recoveries: recoveries))
    }
}

/// The menu-bar item's menu: how many requests wait, and the way to the window.
struct MenuBarContent: View {
    let model: MacAppModel
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        Text(Ja.MenuBar.status(model.menuState))
        if let note = Ja.MenuBar.note(model.menuState) {
            Text(note)
        }
        // A recovery that was started is said here too, as on the desk: the owner can cancel it only while it waits.
        ForEach(model.recoveryLines, id: \.self) { Text($0) }
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
