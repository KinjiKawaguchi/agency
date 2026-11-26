import Foundation
import Testing

/// What a newer node says must not break an older app (`docs/compatibility.md`: readers ignore what they do not know). An enum of
/// words that the apps decode has to read a word it does not know as its own `unknown`, never fail the whole answer and never
/// take it for a word it does know. Reading the sources: every `Decodable` or `Codable` enum conforms to `OpenWireEnum`, or
/// decodes itself and has an `unknown` case, or is named here with why it is not a word of a node's.
@Suite("Enums the apps decode have a word for the unknown")
struct OpenWireEnumsGuardTests {
    /// What is allowed to stay closed, and why. Anything new fails until it is read and named here.
    private static let allowedClosed: [String: String] = [
        "AgencyKit/Sources/AgencyKit/Push/PushRegistration.swift:PushEnvironment":
            "the app's own value for the push service it registered with, kept on this device and sent to the node, never read from it",
        "AgencyKit/Sources/AgencyModel/HomeNodes/HomeNodeList.swift:Source":
            "kept on this device by the app: where an entry of the home-node list came from",
        "AgencyKit/Sources/AgencyWidget/WidgetSnapshot.swift:Kind":
            "written by the app for its own widget, which ships in the same bundle; the app maps a request of a kind it does not know to `other`",
    ]

    @Test("every enum the apps decode reads a word it does not know, or is named with why it need not")
    func everyDecodedEnum() throws {
        let found = try Self.closedEnums(inSourcesUnder: "AgencyKit/Sources")
        let unnamed = found.filter { Self.allowedClosed[$0] == nil }.sorted()
        let stale = Self.allowedClosed.keys.filter { !found.contains($0) }.sorted()
        #expect(unnamed.isEmpty && stale.isEmpty, Comment(rawValue: "closed and not named: \(unnamed); named but not closed any more: \(stale)"))
    }

    @Test("the scan sees a closed enum when there is one, and not an open one")
    func scanSees() {
        func closed(_ source: String) -> [String] { Self.closedEnums(in: source) }
        #expect(closed("public enum Kind: String, Decodable, Sendable { case a, b }") == ["Kind"])
        #expect(closed("enum Kind: String, Codable {\n    case a\n}") == ["Kind"])
        #expect(closed("enum Kind: String,\n    Decodable { case a }") == ["Kind"], "a conformance on the next line")
        #expect(closed("enum State: String, Decodable { case a, unknown }") == ["State"], "an unknown case that decoding never reaches is no guard")
        #expect(closed("enum State: Decodable { case a, unknown(String)\n init(from d: any Decoder) throws { self = .unknown(\"\") } }").isEmpty)
        #expect(closed("public enum Kind: String, OpenWireEnum { case a, unknown }").isEmpty)
        #expect(closed("enum Kind: String, Sendable { case a }").isEmpty, "not decoded")
        #expect(closed("// enum Kind: String, Decodable { case a }").isEmpty, "a comment")
        #expect(closed("enum Outer: Decodable { case a, unknown(String)\n init(from d: any Decoder) throws { }\n enum Inner: String, Decodable { case x } }") == ["Inner"], "an inner enum is its own")
    }

    // MARK: Reading the sources

    private static func root(from file: String = #filePath) -> URL {
        // Tests/AgencyKitTests/<file>: four levels up is `apple/`.
        var url = URL(fileURLWithPath: file)
        for _ in 0..<4 { url.deleteLastPathComponent() }
        return url
    }

    /// `path:Name` of each closed enum under a directory.
    private static func closedEnums(inSourcesUnder directory: String) throws -> Set<String> {
        let root = root()
        var found: Set<String> = []
        let base = root.appendingPathComponent(directory)
        let files = try #require(FileManager.default.enumerator(at: base, includingPropertiesForKeys: nil))
        for case let url as URL in files where url.pathExtension == "swift" {
            let relative = String(url.path.dropFirst(root.path.count + 1))
            for name in closedEnums(in: try String(contentsOf: url, encoding: .utf8)) { found.insert("\(relative):\(name)") }
        }
        return found
    }

    private static let declaration = try! NSRegularExpression(pattern: #"\benum\s+(\w+)\s*:\s*([^{]*)\{"#)
    private static let unknownCase = try! NSRegularExpression(pattern: #"\bcase\s[^\n]*\bunknown\b"#)

    /// The names of the enums in a source that are decoded and neither conform to `OpenWireEnum` nor decode themselves with an
    /// `unknown` case.
    static func closedEnums(in source: String) -> [String] {
        let text = source.split(separator: "\n", omittingEmptySubsequences: false).filter { !$0.trimmingCharacters(in: .whitespaces).hasPrefix("//") }.joined(separator: "\n")
        let whole = NSRange(text.startIndex..., in: text)
        return declaration.matches(in: text, range: whole).compactMap { match -> String? in
            let name = String(text[Range(match.range(at: 1), in: text)!])
            let conformances = String(text[Range(match.range(at: 2), in: text)!])
            guard conformances.contains("Decodable") || conformances.contains("Codable") else { return nil }
            if conformances.contains("OpenWireEnum") { return nil }
            let body = Self.body(of: text, from: Range(match.range, in: text)!.upperBound)
            let decodesItself = body.contains("init(from")
            let hasUnknown = unknownCase.firstMatch(in: body, range: NSRange(body.startIndex..., in: body)) != nil
            return decodesItself && hasUnknown ? nil : name
        }
    }

    /// The text up to the brace that closes the one just opened, with the inner enums' own text left in (they are found on their own).
    private static func body(of text: String, from start: String.Index) -> String {
        var depth = 1
        var end = start
        while end < text.endIndex, depth > 0 {
            if text[end] == "{" { depth += 1 } else if text[end] == "}" { depth -= 1 }
            end = text.index(after: end)
        }
        return String(text[start..<end])
    }
}
