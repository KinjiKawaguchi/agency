import Foundation
import Testing

/// A device is not removed while a decision it signed is kept on the phone, waiting for the node (agency#447, agency#443). The rule lives in
/// `DevicesStore` and is tested there with a hook the test passes; the line that gives the app's store the answer of the app's outbox is in the
/// app target, where no test reaches it. A hook that always says "nothing is kept" passes every other test, so the source is read here.
@Suite("The device removal asks the outbox")
struct KeptDecisionsGuardTests {
    private static func root(from file: String = #filePath) -> URL {
        var url = URL(fileURLWithPath: file)
        for _ in 0..<4 { url.deleteLastPathComponent() }
        return url
    }

    /// The body of the `hasKeptDecisions:` closure that a `DevicesStore(` call is given, without comments; `nil` when there is none.
    static func hook(in text: String) -> String? {
        let code = text.split(separator: "\n", omittingEmptySubsequences: false)
            .filter { !$0.trimmingCharacters(in: .whitespaces).hasPrefix("//") }.joined(separator: "\n")
        guard let call = code.range(of: "DevicesStore(") else { return nil }
        let rest = String(code[call.upperBound...])
        guard let pattern = try? NSRegularExpression(pattern: #"hasKeptDecisions:\s*\{([^}]*)\}"#),
              let match = pattern.firstMatch(in: rest, range: NSRange(rest.startIndex..., in: rest)),
              let body = Range(match.range(at: 1), in: rest) else { return nil }
        return String(rest[body])
    }

    @Test("the app's devices store is given what the app's outbox says is kept")
    func wired() throws {
        let text = try String(contentsOf: Self.root().appendingPathComponent("Shared/Native/NativeSession.swift"), encoding: .utf8)
        let body = try #require(Self.hook(in: text), "the devices store is made without `hasKeptDecisions:`")
        #expect(body.contains("outbox.hasSignedEntries"), "the hook does not ask the outbox: \(body)")
    }

    @Test("the scan sees a hook that does not ask the outbox, and one that does")
    func scanSees() {
        #expect(Self.hook(in: "DevicesStore(source: c, hasKeptDecisions: { _ in false })") == " _ in false ")
        #expect(Self.hook(in: "DevicesStore(\n    source: c, hasKeptDecisions: { _ in outbox.hasSignedEntries }\n)")?.contains("outbox.hasSignedEntries") == true)
        #expect(Self.hook(in: "DevicesStore(source: c)") == nil, "no hook at all")
        #expect(Self.hook(in: "// DevicesStore(hasKeptDecisions: { outbox.hasSignedEntries })\nDevicesStore(source: c)") == nil, "a comment is not a hook")
    }
}
