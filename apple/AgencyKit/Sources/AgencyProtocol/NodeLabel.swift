import Foundation

/// A label the node gives (an org's, a team's, a member's, a KPI's, a device's, whom a request is answered for) is shown with the
/// characters that can reorder or hide what is around it replaced by `?`: a bidirectional override in a label can reorder the line
/// it sits on, on the screen where the owner decides, as it can in a terminal. The node refuses them in a new label and prints an older
/// one's as `?` in its own text; a label stored before that can still hold them, and every client shows a label with the same set
/// replaced. That set is the control characters (Unicode category Cc), the bidirectional controls (U+061C, U+200E, U+200F, U+202A to
/// U+202E, U+2066 to U+2069), the zero-width space (U+200B), the word joiner (U+2060) and the byte order mark (U+FEFF). The zero-width
/// joiner and non-joiner and the emoji tag characters stay: they are part of how some scripts and emoji are written.
public enum NodeLabel {
    /// `text` as it is shown: each character of the set above is a `?`.
    public static func shown(_ text: String) -> String {
        guard text.unicodeScalars.contains(where: isHidden) else { return text }
        var scalars = String.UnicodeScalarView()
        for scalar in text.unicodeScalars { scalars.append(isHidden(scalar) ? "?" : scalar) }
        return String(scalars)
    }

    /// Whether a character of a label is shown as `?`.
    public static func isHidden(_ scalar: Unicode.Scalar) -> Bool {
        switch scalar.value {
        case 0x061C, 0x200B, 0x200E, 0x200F, 0x2060, 0xFEFF, 0x202A...0x202E, 0x2066...0x2069: true
        default: scalar.properties.generalCategory == .control
        }
    }
}

/// A label the node gives, read for showing: its hidden characters are `?` from the moment it is decoded, so that no screen can show
/// one by forgetting. Everything else reads as the node wrote it.
@propertyWrapper
public struct Shown: Decodable, Equatable, Sendable {
    private var text: String

    /// Set only from inside the type that holds it (`private(set)`), and shown the same way whatever sets it.
    public var wrappedValue: String {
        get { text }
        set { text = NodeLabel.shown(newValue) }
    }

    public init(wrappedValue: String) {
        text = NodeLabel.shown(wrappedValue)
    }

    public init(from decoder: any Decoder) throws {
        text = NodeLabel.shown(try decoder.singleValueContainer().decode(String.self))
    }
}
