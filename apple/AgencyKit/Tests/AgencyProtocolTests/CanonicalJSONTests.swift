import Foundation
import Testing
@testable import AgencyProtocol

/// Cases the shared vectors do not cover yet. The expected text is what `serde_json` writes.
@Suite("CanonicalJSON")
struct CanonicalJSONTests {
    private func encode(_ value: JSONValue) -> String {
        try! CanonicalJSON.encode(value)
    }

    @Test("keys sort by UTF-8 bytes, which is not the UTF-16 order")
    func keyOrder() {
        // U+FF5E is EF BD 9E in UTF-8 and FF5E in UTF-16; U+10000 is F0 90 80 80 and D800 DC00.
        #expect(encode(["\u{10000}": 1, "\u{FF5E}": 2, "z": 3, "é": 4, "B": 5, "a": 6])
            == "{\"B\":5,\"a\":6,\"z\":3,\"é\":4,\"\u{FF5E}\":2,\"\u{10000}\":1}")
    }

    @Test("null members are left out of objects but kept in arrays")
    func nulls() {
        #expect(encode(["a": nil, "b": [nil, 1], "c": ["d": nil]]) == "{\"b\":[null,1],\"c\":{}}")
    }

    @Test("a repeated key is refused, also when the keys differ only in how they are normalized")
    func duplicateKeys() {
        #expect(throws: CanonicalJSONError.duplicateKey("a")) {
            try CanonicalJSON.encode(.object([.init("a", 1), .init("a", 2)]))
        }
        let composed = "\u{E9}"
        let decomposed = "e\u{301}"
        let both = try? CanonicalJSON.encode(.object([.init(composed, 1), .init(decomposed, 2)]))
        #expect(both == "{\"e\u{301}\":2,\"\u{E9}\":1}", "different scalars are different keys and stay as written")
    }

    @Test("strings keep their scalars: no normalization, and only what serde_json escapes is escaped")
    func strings() {
        #expect(encode("a/b") == "\"a/b\"")
        #expect(encode("\u{7F}") == "\"\u{7F}\"")
        #expect(encode("\u{2028}\u{2029}") == "\"\u{2028}\u{2029}\"")
        #expect(encode("日本語🙂") == "\"日本語🙂\"")
        #expect(encode("e\u{301}") == "\"e\u{301}\"")
        #expect(encode("say \"hi\" \\ ok") == "\"say \\\"hi\\\" \\\\ ok\"")
    }

    @Test("control characters use the short escapes, or \\u00xx in lowercase hex")
    func controls() {
        #expect(encode("\u{08}\u{0C}\n\r\t") == "\"\\b\\f\\n\\r\\t\"")
        #expect(encode("\u{00}\u{01}\u{1F}\u{0B}\u{1A}") == "\"\\u0000\\u0001\\u001f\\u000b\\u001a\"")
    }

    @Test("integers, booleans and empty containers")
    func scalars() {
        #expect(encode([0, -7, 9_007_199_254_740_993, true, false, [], [:]]) == "[0,-7,9007199254740993,true,false,[],{}]")
    }
}
