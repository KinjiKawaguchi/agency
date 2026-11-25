import AgencyProtocol
import Foundation

/// A small JSON reader for the test vectors. `JSONSerialization` would do, but it hands back
/// dictionaries (no member order) and numbers that cannot tell a boolean from a 1; this keeps both,
/// and keeps every string's scalars as they are in the file.
struct JSONParser {
    struct Failure: Error { let offset: Int }

    private let bytes: [UInt8]
    private var index = 0

    static func parse(_ data: Data) throws -> JSONValue {
        var parser = JSONParser(bytes: [UInt8](data))
        let value = try parser.value()
        parser.skipSpace()
        guard parser.index == parser.bytes.count else { throw Failure(offset: parser.index) }
        return value
    }

    private init(bytes: [UInt8]) { self.bytes = bytes }

    private mutating func skipSpace() {
        while index < bytes.count, [0x20, 0x0A, 0x0D, 0x09].contains(bytes[index]) { index += 1 }
    }

    private mutating func value() throws -> JSONValue {
        skipSpace()
        guard index < bytes.count else { throw Failure(offset: index) }
        switch bytes[index] {
        case UInt8(ascii: "{"): return try object()
        case UInt8(ascii: "["): return try array()
        case UInt8(ascii: "\""): return .string(try string())
        case UInt8(ascii: "t"): try expect("true"); return .bool(true)
        case UInt8(ascii: "f"): try expect("false"); return .bool(false)
        case UInt8(ascii: "n"): try expect("null"); return .null
        default: return try number()
        }
    }

    private mutating func expect(_ word: String) throws {
        let letters = Array(word.utf8)
        guard bytes.count - index >= letters.count, Array(bytes[index..<index + letters.count]) == letters else {
            throw Failure(offset: index)
        }
        index += letters.count
    }

    private mutating func number() throws -> JSONValue {
        let start = index
        if index < bytes.count, bytes[index] == UInt8(ascii: "-") { index += 1 }
        while index < bytes.count, (48...57).contains(bytes[index]) { index += 1 }
        guard let number = Int(String(decoding: bytes[start..<index], as: UTF8.self)) else { throw Failure(offset: start) }
        return .integer(number)
    }

    private mutating func array() throws -> JSONValue {
        index += 1
        var items: [JSONValue] = []
        skipSpace()
        if bytes[index] == UInt8(ascii: "]") { index += 1; return .array(items) }
        while true {
            items.append(try value())
            skipSpace()
            if bytes[index] == UInt8(ascii: ",") { index += 1; continue }
            guard bytes[index] == UInt8(ascii: "]") else { throw Failure(offset: index) }
            index += 1
            return .array(items)
        }
    }

    private mutating func object() throws -> JSONValue {
        index += 1
        var members: [JSONValue.Member] = []
        skipSpace()
        if bytes[index] == UInt8(ascii: "}") { index += 1; return .object(members) }
        while true {
            skipSpace()
            guard bytes[index] == UInt8(ascii: "\"") else { throw Failure(offset: index) }
            let key = try string()
            skipSpace()
            guard bytes[index] == UInt8(ascii: ":") else { throw Failure(offset: index) }
            index += 1
            members.append(.init(key, try value()))
            skipSpace()
            if bytes[index] == UInt8(ascii: ",") { index += 1; continue }
            guard bytes[index] == UInt8(ascii: "}") else { throw Failure(offset: index) }
            index += 1
            return .object(members)
        }
    }

    private mutating func string() throws -> String {
        index += 1
        var scalars = String.UnicodeScalarView()
        var raw: [UInt8] = []
        func flush() { scalars.append(contentsOf: String(decoding: raw, as: UTF8.self).unicodeScalars); raw = [] }
        while index < bytes.count {
            let byte = bytes[index]
            index += 1
            if byte == UInt8(ascii: "\"") { flush(); return String(scalars) }
            guard byte == UInt8(ascii: "\\") else { raw.append(byte); continue }
            flush()
            scalars.append(try escape())
        }
        throw Failure(offset: index)
    }

    private mutating func escape() throws -> Unicode.Scalar {
        let code = bytes[index]
        index += 1
        switch code {
        case UInt8(ascii: "\""): return "\""
        case UInt8(ascii: "\\"): return "\\"
        case UInt8(ascii: "/"): return "/"
        case UInt8(ascii: "b"): return "\u{08}"
        case UInt8(ascii: "f"): return "\u{0C}"
        case UInt8(ascii: "n"): return "\n"
        case UInt8(ascii: "r"): return "\r"
        case UInt8(ascii: "t"): return "\t"
        case UInt8(ascii: "u"):
            var unit = try hex4()
            if (0xD800..<0xDC00).contains(unit), bytes[index] == UInt8(ascii: "\\"), bytes[index + 1] == UInt8(ascii: "u") {
                index += 2
                let low = try hex4()
                unit = 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00)
            }
            guard let scalar = Unicode.Scalar(unit) else { throw Failure(offset: index) }
            return scalar
        default: throw Failure(offset: index)
        }
    }

    private mutating func hex4() throws -> UInt32 {
        guard index + 4 <= bytes.count, let unit = UInt32(String(decoding: bytes[index..<index + 4], as: UTF8.self), radix: 16) else {
            throw Failure(offset: index)
        }
        index += 4
        return unit
    }
}

extension JSONValue {
    /// The member named `key` of an object.
    subscript(key: String) -> JSONValue? {
        guard case .object(let members) = self else { return nil }
        return members.first { Array($0.key.utf8) == Array(key.utf8) }?.value
    }

    var text: String? {
        if case .string(let text) = self { text } else { nil }
    }

    var items: [JSONValue] {
        if case .array(let items) = self { items } else { [] }
    }
}
