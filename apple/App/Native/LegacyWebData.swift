import Foundation
import WebKit

/// Earlier versions of the app showed the web client in a web view, which kept the Access cookie (a copy
/// of the token) and the web client's own storage (its device key) in the app's website data. Nothing
/// reads them now, so they are removed, once.
enum LegacyWebData {
    private static let doneKey = "legacyWebDataRemoved"

    @MainActor
    static func removeOnce(defaults: UserDefaults = .standard) async {
        guard !defaults.bool(forKey: doneKey) else { return }
        await WKWebsiteDataStore.default().removeData(ofTypes: WKWebsiteDataStore.allWebsiteDataTypes(), modifiedSince: .distantPast)
        defaults.set(true, forKey: doneKey)
    }
}
