import AgencyAPI
import AgencyKit

/// Whether this app and its home node still fit each other, from what the node says of itself (`GET /api/about`, `docs/compatibility.md`,
/// "Node and apps"). The app compares what it speaks and what it is with what the node serves, and says which of the two is to be updated;
/// it never decides by a version of its own making.
public enum NodeCompatibility: Equatable, Sendable {
    case compatible
    /// The node speaks an older Client API than the app: the node is updated.
    case nodeTooOld
    /// The node no longer serves this app, by its Client API range or by its floor for this platform: the app is updated.
    case appTooOld

    /// - Parameters:
    ///   - about: what the node said; `nil` when it has not, which says nothing.
    ///   - clientApi: the Client API version the app speaks.
    ///   - appVersion: the app's own version, `nil` when it has none in the form the node reads.
    ///   - platform: the app's platform, whose floor (`oldestApp`) is the one that counts.
    public static func judge(about: About?, clientApi: Int, appVersion: String?, platform: ClientIdentity.Platform) -> NodeCompatibility {
        guard let about else { return .compatible }
        if let range = about.clientApi {
            if clientApi < range.oldest { return .appTooOld }
            if clientApi > range.current { return .nodeTooOld }
        }
        if let floor = about.oldestApp?[platform.rawValue].flatMap(Version.init), let own = appVersion.flatMap(Version.init), own < floor {
            return .appTooOld
        }
        return .compatible
    }

    /// What the viewer is told, or `nil` when nothing is wrong. The app is always the viewer's to update; the node is the owner's, so a guest
    /// of someone else's node is told to tell the owner.
    public func notice(isGuest: Bool) -> String? {
        switch self {
        case .compatible: nil
        case .nodeTooOld: isGuest ? Ja.Compatibility.nodeTooOldForGuest : Ja.Compatibility.nodeTooOld
        case .appTooOld: Ja.Compatibility.appTooOld
        }
    }
}

/// What the desk says above its requests: the first thing that is wrong, the versions before the key and the sign-in.
public enum DeskNotice {
    public static func first(compatibility: NodeCompatibility, isGuest: Bool, then others: [String?]) -> String? {
        ([compatibility.notice(isGuest: isGuest)] + others).compactMap { $0 }.first
    }
}

/// A version of numbers separated by dots, compared number by number, with a missing number as 0 (`1.0` is `1.0.0`).
struct Version: Comparable {
    private let numbers: [Int]

    /// `nil` for anything that is not numbers separated by dots, so that a floor the app cannot read is no floor.
    init?(_ text: String) {
        let parts = text.split(separator: ".", omittingEmptySubsequences: false)
        let numbers = parts.compactMap { part in part.allSatisfy { $0.isASCII && $0.isNumber } && part.count <= 9 ? Int(part) : nil }
        guard !parts.isEmpty, numbers.count == parts.count else { return nil }
        self.numbers = numbers
    }

    static func < (left: Version, right: Version) -> Bool {
        let length = max(left.numbers.count, right.numbers.count)
        for index in 0..<length {
            let (a, b) = (index < left.numbers.count ? left.numbers[index] : 0, index < right.numbers.count ? right.numbers[index] : 0)
            if a != b { return a < b }
        }
        return false
    }

    static func == (left: Version, right: Version) -> Bool { !(left < right) && !(right < left) }
}
