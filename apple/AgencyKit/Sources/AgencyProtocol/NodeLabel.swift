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

    /// `text` as a person reads a paragraph of it: the same set as `shown`, except that a line break, a carriage return and a tab stay, since a
    /// title's context, a message or a document is written in lines. A text from an agent can hold the same characters that reorder a label.
    public static func shownText(_ text: String) -> String {
        guard text.unicodeScalars.contains(where: isHiddenInText) else { return text }
        var scalars = String.UnicodeScalarView()
        for scalar in text.unicodeScalars { scalars.append(isHiddenInText(scalar) ? "?" : scalar) }
        return String(scalars)
    }

    /// Whether a character of a text is shown as `?`: a character of a label's set, but not a tab, a line feed or a carriage return.
    public static func isHiddenInText(_ scalar: Unicode.Scalar) -> Bool {
        scalar.value != 0x09 && scalar.value != 0x0A && scalar.value != 0x0D && isHidden(scalar)
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

/// `Shown` for a field the app also sends back to the node or signs: it keeps what the node wrote (`$field`), because a display transform
/// must not change what is signed, or the signature covers bytes nobody sent. The raw value is for sending, never for drawing, and only
/// the fields that are sent back keep it.
@propertyWrapper
public struct ShownWithRaw: Decodable, Equatable, Sendable {
    private var raw: String
    private var text: String

    public var wrappedValue: String {
        get { text }
        set {
            raw = newValue
            text = NodeLabel.shown(newValue)
        }
    }

    public var projectedValue: String { raw }

    public init(wrappedValue: String) {
        raw = wrappedValue
        text = NodeLabel.shown(wrappedValue)
    }

    public init(from decoder: any Decoder) throws {
        raw = try decoder.singleValueContainer().decode(String.self)
        text = NodeLabel.shown(raw)
    }
}

/// The same for a label that may be missing: it is `nil` when the node leaves it out or says `null`, and shown otherwise.
@propertyWrapper
public struct ShownIfPresent: Decodable, Equatable, Sendable {
    private var text: String?

    public var wrappedValue: String? {
        get { text }
        set { text = newValue.map(NodeLabel.shown) }
    }

    public init(wrappedValue: String?) {
        text = wrappedValue.map(NodeLabel.shown)
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.singleValueContainer()
        text = container.decodeNil() ? nil : NodeLabel.shown(try container.decode(String.self))
    }
}

/// The same for a map of labels by id that may be missing (whom each request is answered for): the values are shown, the keys (ids)
/// are as the node wrote them, and a missing map is `nil`.
@propertyWrapper
public struct ShownValuesIfPresent: Decodable, Equatable, Sendable {
    private var values: [String: String]?

    public var wrappedValue: [String: String]? {
        get { values }
        set { values = newValue.map { $0.mapValues(NodeLabel.shown) } }
    }

    public init(wrappedValue: [String: String]?) {
        values = wrappedValue.map { $0.mapValues(NodeLabel.shown) }
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.singleValueContainer()
        values = container.decodeNil() ? nil : try container.decode([String: String].self).mapValues(NodeLabel.shown)
    }
}

extension KeyedDecodingContainer {
    /// A map the node leaves out is `nil`, as an optional field is, though its wrapper is not optional.
    public func decode(_ type: ShownValuesIfPresent.Type, forKey key: Key) throws -> ShownValuesIfPresent {
        try decodeIfPresent(type, forKey: key) ?? ShownValuesIfPresent(wrappedValue: nil)
    }
}

extension KeyedDecodingContainer {
    /// A label the node leaves out is `nil`, as an optional field is, though its wrapper is not optional.
    public func decode(_ type: ShownIfPresent.Type, forKey key: Key) throws -> ShownIfPresent {
        try decodeIfPresent(type, forKey: key) ?? ShownIfPresent(wrappedValue: nil)
    }
}

/// A text the node gives that is written in lines (a request's context and document, a message, a comment, what an agent says it does):
/// shown with the same hidden characters as `?`, and with its line breaks and tabs kept.
@propertyWrapper
public struct ShownText: Decodable, Equatable, Sendable {
    private var text: String

    public var wrappedValue: String {
        get { text }
        set { text = NodeLabel.shownText(newValue) }
    }

    public init(wrappedValue: String) {
        text = NodeLabel.shownText(wrappedValue)
    }

    public init(from decoder: any Decoder) throws {
        text = NodeLabel.shownText(try decoder.singleValueContainer().decode(String.self))
    }
}

/// `ShownText` for a field the app also sends back: it keeps what the node wrote (`$field`), for sending and never for drawing.
@propertyWrapper
public struct ShownTextWithRaw: Decodable, Equatable, Sendable {
    private var raw: String
    private var text: String

    public var wrappedValue: String {
        get { text }
        set {
            raw = newValue
            text = NodeLabel.shownText(newValue)
        }
    }

    public var projectedValue: String { raw }

    public init(wrappedValue: String) {
        raw = wrappedValue
        text = NodeLabel.shownText(wrappedValue)
    }

    public init(from decoder: any Decoder) throws {
        raw = try decoder.singleValueContainer().decode(String.self)
        text = NodeLabel.shownText(raw)
    }
}

/// The same for a text that may be missing.
@propertyWrapper
public struct ShownTextIfPresent: Decodable, Equatable, Sendable {
    private var text: String?

    public var wrappedValue: String? {
        get { text }
        set { text = newValue.map(NodeLabel.shownText) }
    }

    public init(wrappedValue: String?) {
        text = wrappedValue.map(NodeLabel.shownText)
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.singleValueContainer()
        text = container.decodeNil() ? nil : NodeLabel.shownText(try container.decode(String.self))
    }
}

extension KeyedDecodingContainer {
    /// A text the node leaves out is `nil`, as an optional field is.
    public func decode(_ type: ShownTextIfPresent.Type, forKey key: Key) throws -> ShownTextIfPresent {
        try decodeIfPresent(type, forKey: key) ?? ShownTextIfPresent(wrappedValue: nil)
    }
}

