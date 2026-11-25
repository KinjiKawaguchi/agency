import Foundation
import Testing
@testable import AgencyProtocol

/// What the shared vectors (`VectorTests`) do not cover: the encoder's own refusals and a few scalars.
@Suite("CanonicalJSON")
struct CanonicalJSONTests {
    private func encode(_ value: JSONValue) -> String {
        try! CanonicalJSON.encode(value)
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

    @Test("integers, booleans and empty containers")
    func scalars() {
        #expect(encode([0, -7, 9_007_199_254_740_993, true, false, [], [:]]) == "[0,-7,9007199254740993,true,false,[],{}]")
    }
}
