import Foundation
import Testing
@testable import AgencyKit

@Suite("NativeMessage")
struct NativeMessageTests {
    private func message(count: Any?) -> [String: Any] {
        var body: [String: Any] = ["type": "badge"]
        if let count { body["count"] = count }
        return body
    }

    @Test("a whole number in range is a badge")
    func acceptsWholeNumbers() {
        #expect(NativeMessage(body: message(count: 0)) == .badge(0))
        #expect(NativeMessage(body: message(count: 1)) == .badge(1))
        #expect(NativeMessage(body: message(count: 12)) == .badge(12))
        #expect(NativeMessage(body: message(count: 9999)) == .badge(9999))
        #expect(NativeMessage(body: message(count: 3.0)) == .badge(3))
    }

    @Test("a number out of range, a fraction, or another type is ignored")
    func rejectsOtherCounts() {
        #expect(NativeMessage(body: message(count: 10000)) == nil)
        #expect(NativeMessage(body: message(count: -1)) == nil)
        #expect(NativeMessage(body: message(count: 3.5)) == nil)
        #expect(NativeMessage(body: message(count: Double.nan)) == nil)
        #expect(NativeMessage(body: message(count: Double.infinity)) == nil)
        #expect(NativeMessage(body: message(count: "5")) == nil)
        #expect(NativeMessage(body: message(count: [1])) == nil)
        #expect(NativeMessage(body: message(count: nil)) == nil)
    }

    @Test("a boolean is not the number 0 or 1")
    func rejectsBooleans() {
        #expect(NativeMessage(body: message(count: true)) == nil)
        #expect(NativeMessage(body: message(count: false)) == nil)
    }

    @Test("a body that is not exactly a known message is ignored")
    func rejectsOtherBodies() {
        #expect(NativeMessage(body: ["type": "open", "count": 1]) == nil)
        #expect(NativeMessage(body: ["count": 1]) == nil)
        #expect(NativeMessage(body: ["type": 1, "count": 1]) == nil)
        #expect(NativeMessage(body: "badge") == nil)
        #expect(NativeMessage(body: [1, 2]) == nil)
    }
}

#if canImport(JavaScriptCore)
import JavaScriptCore

/// What WebKit really hands over: JavaScript values converted to Foundation objects.
@Suite("NativeMessage from JavaScript values")
struct NativeMessageJavaScriptTests {
    private func parse(_ literal: String) -> NativeMessage? {
        let context = JSContext()!
        guard let body = context.evaluateScript("(\(literal))")?.toObject() else { return nil }
        return NativeMessage(body: body)
    }

    @Test("numbers", arguments: [
        ("{ type: 'badge', count: 0 }", NativeMessage.badge(0)),
        ("{ type: 'badge', count: 1 }", NativeMessage.badge(1)),
        ("{ type: 'badge', count: 42 }", NativeMessage.badge(42)),
        ("{ type: 'badge', count: 9999 }", NativeMessage.badge(9999)),
    ])
    func numbers(literal: String, expected: NativeMessage) {
        #expect(parse(literal) == expected)
    }

    @Test("booleans, fractions, and the rest are ignored", arguments: [
        "{ type: 'badge', count: true }",
        "{ type: 'badge', count: false }",
        "{ type: 'badge', count: 1.5 }",
        "{ type: 'badge', count: NaN }",
        "{ type: 'badge', count: -3 }",
        "{ type: 'badge', count: 10000 }",
        "{ type: 'badge', count: '7' }",
        "{ type: 'badge', count: null }",
        "{ type: 'badge' }",
        "{ type: 'other', count: 1 }",
    ])
    func rejects(literal: String) {
        #expect(parse(literal) == nil)
    }
}
#endif
