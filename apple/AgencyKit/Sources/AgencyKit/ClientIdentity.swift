import Foundation

/// What the app tells the node about itself on every Client API call, as the `x-agency-client` header, so that a node
/// knows which app versions its owners use (discovery, agency#318): `ios/0.3.1 (202611261830)` or `macos/0.3.1 (…)`.
///
/// The version is the bundle's `CFBundleShortVersionString` and the build its `CFBundleVersion` (the 12-digit UTC time
/// of a TestFlight build). The node's reader is strict and ignores the whole header when any part is outside its form, so
/// the app sends only what is in it: the version exactly three numbers (`MAJOR.MINOR.PATCH`, one to six digits each), the
/// build only digits and no more than an unsigned 64-bit integer. With no version in that form the app says nothing; with
/// no build in that form it says the version alone, so that a build like `1.2.3` does not cost the version too.
public enum ClientIdentity {
    public enum Platform: String, Sendable {
        case ios
        case macos
    }

    public static let header = "x-agency-client"

    private static func isDigits(_ text: Substring) -> Bool {
        !text.isEmpty && text.allSatisfy { $0.isASCII && $0.isNumber }
    }

    private static func version(_ text: String?) -> String? {
        guard let text else { return nil }
        let parts = text.split(separator: ".", omittingEmptySubsequences: false)
        guard parts.count == 3, parts.allSatisfy({ isDigits($0) && $0.count <= 6 }) else { return nil }
        return text
    }

    private static func build(_ text: String?) -> String? {
        guard let text, isDigits(Substring(text)), UInt64(text) != nil else { return nil }
        return text
    }

    /// The header's value, or `nil` when there is no version to tell.
    public static func value(platform: Platform, version text: String?, build number: String?) -> String? {
        guard let version = version(text) else { return nil }
        let short = "\(platform.rawValue)/\(version)"
        guard let build = build(number) else { return short }
        // At most 6 + 6 + 6 digits and dots, the platform and a build of at most 20 digits: under the node's 64 characters.
        return "\(short) (\(build))"
    }

    /// This app's: its own bundle's version and build, on the platform it was built for.
    public static func current(bundle: Bundle = .main) -> String? {
        #if os(macOS)
        let platform = Platform.macos
        #else
        let platform = Platform.ios
        #endif
        let info = bundle.infoDictionary
        return value(platform: platform, version: info?["CFBundleShortVersionString"] as? String, build: info?["CFBundleVersion"] as? String)
    }
}
