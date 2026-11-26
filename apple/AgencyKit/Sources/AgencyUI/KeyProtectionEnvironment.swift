#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

private struct KeyProtectionKey: EnvironmentKey {
    static let defaultValue = KeyProtection.faceID
}

extension EnvironmentValues {
    /// How this device's key asks the person to confirm a signature, for the words and the symbol the screens use.
    public var keyProtection: KeyProtection {
        get { self[KeyProtectionKey.self] }
        set { self[KeyProtectionKey.self] = newValue }
    }
}
#endif
