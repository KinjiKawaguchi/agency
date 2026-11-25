import Foundation

public enum CanonicalJSONError: Error, Equatable, Sendable {
    /// Two members of one object have the same key (the same UTF-8 bytes).
    case duplicateKey(String)
}

/// Canonical JSON, as `docs/protocol.md` defines it and `agency-proto` writes it: object keys sorted
/// by their UTF-8 bytes, no whitespace, strings escaped as standard JSON, integers only, and object
/// members whose value is null left out. The bytes of a signed text must be the same here as on the
/// node that checks the signature, so `testdata/vectors.json` pins them.
public enum CanonicalJSON {
    public static func encode(_ value: JSONValue) throws -> String {
        var output = ""
        try write(value, to: &output)
        return output
    }

    private static func write(_ value: JSONValue, to output: inout String) throws {
        switch value {
        case .null: output += "null"
        case .bool(let flag): output += flag ? "true" : "false"
        case .integer(let number): output += String(number)
        case .string(let text): writeString(text, to: &output)
        case .array(let items):
            output += "["
            for (index, item) in items.enumerated() {
                if index > 0 { output += "," }
                try write(item, to: &output)
            }
            output += "]"
        case .object(let members):
            try writeObject(members, to: &output)
        }
    }

    private static func writeObject(_ members: [JSONValue.Member], to output: inout String) throws {
        let kept = members.filter { $0.value != .null }
        let sorted = kept.sorted { Array($0.key.utf8).lexicographicallyPrecedes(Array($1.key.utf8)) }
        for (first, second) in zip(sorted, sorted.dropFirst()) where Array(first.key.utf8) == Array(second.key.utf8) {
            throw CanonicalJSONError.duplicateKey(first.key)
        }
        output += "{"
        for (index, member) in sorted.enumerated() {
            if index > 0 { output += "," }
            writeString(member.key, to: &output)
            output += ":"
            try write(member.value, to: &output)
        }
        output += "}"
    }

    /// Escapes the way `serde_json` does: the quote, the backslash and the control characters below
    /// U+0020 (`\b \f \n \r \t`, or `\u00xx` in lowercase hex). Everything else, including `/`,
    /// U+007F, U+2028 and every non-ASCII character, is written as it is.
    private static func writeString(_ text: String, to output: inout String) {
        output += "\""
        for scalar in text.unicodeScalars {
            switch scalar {
            case "\"": output += "\\\""
            case "\\": output += "\\\\"
            case "\u{08}": output += "\\b"
            case "\u{0C}": output += "\\f"
            case "\n": output += "\\n"
            case "\r": output += "\\r"
            case "\t": output += "\\t"
            case _ where scalar.value < 0x20:
                let hex = String(scalar.value, radix: 16)
                output += "\\u" + String(repeating: "0", count: 4 - hex.count) + hex
            default: output.unicodeScalars.append(scalar)
            }
        }
        output += "\""
    }
}
