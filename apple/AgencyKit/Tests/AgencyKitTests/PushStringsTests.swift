import Foundation
import Testing

/// The alert strings of `apple/Shared/<language>.lproj/Localizable.strings`: the node names them
/// (`docs/protocol.md`, "Push") and the app ships them, in every language the app has.
@Suite("Push alert strings")
struct PushStringsTests {
    /// The strings the node names, and how many arguments each takes.
    static let expected: [String: Int] = [
        "PUSH_DEVICE_WAITING": 1, "PUSH_DEVICE_WAITING_NO_LABEL": 0,
        "PUSH_DECISION_REFUSED": 1, "PUSH_DECISION_REFUSED_NO_TITLE": 0,
        "PUSH_REQUEST_WAITING_HIDDEN": 0,
    ]

    private func entries(_ language: String) throws -> [String: String] {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 { url.deleteLastPathComponent() }
        url = url.appendingPathComponent("Shared/\(language).lproj/Localizable.strings")
        let text = try String(contentsOf: url, encoding: .utf8)
        var result: [String: String] = [:]
        for line in text.split(separator: "\n") where line.hasPrefix("\"") {
            let parts = line.split(separator: "\"", omittingEmptySubsequences: false).map(String.init)
            // "KEY" = "value";  ->  ["", KEY, " = ", value, ";"]
            guard parts.count == 5 else { continue }
            result[parts[1]] = parts[3]
        }
        return result
    }

    @Test("each language has exactly the strings the node names, each with the number of arguments it is sent", arguments: ["ja", "en"])
    func hasTheStrings(language: String) throws {
        let strings = try entries(language)
        #expect(Set(strings.keys) == Set(Self.expected.keys))
        for (key, count) in Self.expected {
            let placeholders = (strings[key] ?? "").components(separatedBy: "%@").count - 1
            #expect(placeholders == count, "\(key) in \(language) has \(placeholders) placeholders, the node sends \(count)")
        }
    }
}
