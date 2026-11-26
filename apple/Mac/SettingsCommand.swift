import AgencyModel
import SwiftUI

private struct ShellModelKey: FocusedValueKey {
    typealias Value = ShellModel
}

extension FocusedValues {
    /// The shell of the window in front, so a menu command can reach its settings.
    var shellModel: ShellModel? {
        get { self[ShellModelKey.self] }
        set { self[ShellModelKey.self] = newValue }
    }
}

/// "設定…" on ⌘, in the app menu: it opens the settings sheet of the window in front.
struct SettingsCommand: Commands {
    @FocusedValue(\.shellModel) private var model

    var body: some Commands {
        CommandGroup(replacing: .appSettings) {
            Button(Ja.Desk.settings + "…") { model?.showsSettings = true }
                .keyboardShortcut(",")
                .disabled(model == nil)
        }
    }
}
