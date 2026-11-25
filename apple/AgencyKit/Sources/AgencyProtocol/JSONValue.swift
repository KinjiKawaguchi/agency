/// A JSON value as the protocol signs it: integers only, and an object keeps its members in the
/// order they were given (the canonical encoding sorts them).
///
/// An object is a list of members rather than a dictionary, because a Swift dictionary treats two
/// keys that differ only in Unicode normalization as the same key, and the signed bytes must keep
/// each key's scalars exactly.
public enum JSONValue: Equatable, Sendable {
    case null
    case bool(Bool)
    case integer(Int)
    case string(String)
    case array([JSONValue])
    case object([Member])

    public struct Member: Equatable, Sendable {
        public let key: String
        public let value: JSONValue

        public init(_ key: String, _ value: JSONValue) {
            self.key = key
            self.value = value
        }
    }
}

extension JSONValue: ExpressibleByNilLiteral, ExpressibleByBooleanLiteral, ExpressibleByIntegerLiteral,
    ExpressibleByStringLiteral, ExpressibleByArrayLiteral, ExpressibleByDictionaryLiteral {
    public init(nilLiteral: ()) { self = .null }
    public init(booleanLiteral value: Bool) { self = .bool(value) }
    public init(integerLiteral value: Int) { self = .integer(value) }
    public init(stringLiteral value: String) { self = .string(value) }
    public init(arrayLiteral elements: JSONValue...) { self = .array(elements) }
    public init(dictionaryLiteral elements: (String, JSONValue)...) {
        self = .object(elements.map { Member($0.0, $0.1) })
    }
}
