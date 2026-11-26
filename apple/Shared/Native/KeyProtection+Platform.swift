import AgencyModel

extension KeyProtection {
    /// How the key of this platform asks the person to confirm each signature.
    static var platform: KeyProtection {
        #if os(macOS)
        .touchIDOrPassword
        #else
        .faceID
        #endif
    }
}
