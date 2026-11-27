import Foundation
import Testing

/// A label the node gives is shown without the characters that can reorder it (`NodeLabel`, applied when it is decoded). This reads the
/// wire types and fails for a field that is named like a label and is not decoded that way, so that a new one is not forgotten: any
/// `label`, `name`, `member`, `device`, `what`, `from`, `to`, `choiceLabel` or `reason` that is a `String` must be `@Shown` or `@ShownIfPresent`,
/// or be named here with why it is not one.
@Suite("Labels on the wire are all shown without hidden characters")
struct ShownLabelsGuardTests {
    /// What is allowed not to be `@Shown`, and why. Anything new fails until it is read and named here.
    private static let allowed: [String: String] = [
        "AgencyKit/Sources/AgencyAPI/Wire/Metrics.swift:LatencySeries.public let name: String":
            "a route template or a call (`runtime view`) that the node writes itself, never something a person named",
        "AgencyKit/Sources/AgencyAPI/Wire/Evaluation.swift:ParticipantEvaluation.public let name: String":
            "a participant's name: its reader shows it with `NodeLabel.shown` as it decodes it (`ParticipantEvaluation.init(from:)`)",
        "AgencyKit/Sources/AgencyAPI/Wire/RootChanges.swift:RootChangeRef.public let what: String?":
            "the node's words for a held change: its reader shows it with `NodeLabel.shown` (`RootChangeRef.init(from:)`)",
        "AgencyKit/Sources/AgencyAPI/Wire/RootChanges.swift:RootChangeRef.public let member: String?":
            "what a held change names: its reader shows it with `NodeLabel.shown` (`RootChangeRef.init(from:)`)",
        "AgencyKit/Sources/AgencyAPI/Wire/RootChanges.swift:RootChangeRef.public let label: String?":
            "what a held change names: its reader shows it with `NodeLabel.shown` (`RootChangeRef.init(from:)`)",
        "AgencyKit/Sources/AgencyAPI/Wire/RootChanges.swift:RootChangeRef.public let device: String?":
            "what a held change names: its reader shows it with `NodeLabel.shown` (`RootChangeRef.init(from:)`)",
        "AgencyKit/Sources/AgencyAPI/Wire/TeamView.swift:Request.public let from: String":
            "the team runtime's name, which finds the person who has it (`Team.person(named:)`), so it is kept as written; a screen draws `shownFrom` (`DeskItem.drafterName`)",
        "AgencyKit/Sources/AgencyAPI/Wire/TeamView.swift:Request.public let to: String?":
            "the team runtime's name, kept as written to find the person; a screen draws `shownTo` (`DeskItem.deciderName`)",
        "AgencyKit/Sources/AgencyAPI/Wire/TeamView.swift:ThreadMessage.public let from: String":
            "the team runtime's name of who wrote a message, kept as written; a screen draws `shownFrom`",
        "AgencyKit/Sources/AgencyAPI/Wire/Me.swift:GuestRef.public let member: String?":
            "the id of the hosted member, which the app reads to know the viewer is a guest and never shows",
        "AgencyKit/Sources/AgencyAPI/Wire/Effort.swift:Effort.public let from: String": "a calendar day, the period's first: never a label",
        "AgencyKit/Sources/AgencyAPI/Wire/Effort.swift:TeamEffort.public let from: String": "a calendar day, the period's first: never a label",
        "AgencyKit/Sources/AgencyAPI/Wire/Evaluation.swift:Evaluation.public let from: String": "a calendar day, the period's first: never a label",
        "AgencyKit/Sources/AgencyAPI/Wire/Evaluation.swift:TeamEvaluation.public let from: String": "a calendar day, the period's first: never a label",
        "AgencyKit/Sources/AgencyAPI/Wire/Outcomes.swift:Outcomes.public let from: String": "a calendar day, the period's first: never a label",
    ]

    private static let declaration = try! NSRegularExpression(
        pattern: #"public\s+(?:private\(set\)\s+)?(?:let|var)\s+(label|name|member|device|what|from|to|choiceLabel|reason)\s*:\s*String\??(?![\w.])"#
    )

    @Test("every field of the wire types that is named like a label is shown, or is named here with why not")
    func everyLabel() throws {
        let found = try Self.declarations()
        let unshown = found.filter { !$0.value.contains("@Shown") }
        let unnamed = unshown.keys.filter { Self.allowed[$0] == nil }.sorted()
        let stale = Self.allowed.keys.filter { found[$0] == nil || found[$0]?.contains("@Shown") == true }.sorted()
        #expect(unnamed.isEmpty && stale.isEmpty, Comment(rawValue: "not shown and not named: \(unnamed); named but shown or gone: \(stale)"))
        #expect(found.count >= 20, "the scan sees the wire types: \(found.count)")
    }

    @Test("the scan sees a field that is not shown, and one that is")
    func scanSees() {
        func matches(_ line: String) -> Bool { Self.declaration.firstMatch(in: line, range: NSRange(line.startIndex..., in: line)) != nil }
        #expect(matches("    public let label: String"))
        #expect(matches("public struct Z: Decodable { public let label: String }"), "on a line with more")
        #expect(matches("    public let to: String?"))
        #expect(matches("    @Shown public private(set) var label: String"))
        #expect(matches("    @ShownIfPresent public private(set) var choiceLabel: String?"))
        #expect(!matches("    public let id: String"))
        #expect(!matches("    public let from: Instant"))
        #expect(!matches("    public let label: Int"))
    }

    private static let typeDeclaration = try! NSRegularExpression(pattern: #"^(?:public\s+)?(?:final\s+)?(?:struct|class|enum|actor)\s+(\w+)"#)

    /// `file:Type.declaration` (the declaration without its attributes) to the whole declaration line; the type is the last one declared
    /// above it, so that two fields of one name in a file are two entries.
    private static func declarations() throws -> [String: String] {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 { url.deleteLastPathComponent() }
        let wire = url.appendingPathComponent("AgencyKit/Sources/AgencyAPI/Wire")
        var found: [String: String] = [:]
        let files = try #require(FileManager.default.enumerator(at: wire, includingPropertiesForKeys: nil))
        for case let file as URL in files where file.pathExtension == "swift" {
            let relative = "AgencyKit/Sources/AgencyAPI/Wire/" + file.lastPathComponent
            var type = ""
            for line in try String(contentsOf: file, encoding: .utf8).split(separator: "\n") {
                let text = String(line).trimmingCharacters(in: .whitespaces)
                let whole = NSRange(text.startIndex..., in: text)
                // Only a type at the top of the file names the fields below it: a nested one (an enum of words) does not.
                let raw = String(line)
                if let match = typeDeclaration.firstMatch(in: raw, range: NSRange(raw.startIndex..., in: raw)), let name = Range(match.range(at: 1), in: raw) { type = String(raw[name]) }
                guard declaration.firstMatch(in: text, range: whole) != nil else { continue }
                // The declaration itself, without its attributes and with `let` for `var`, as the allowance names it.
                let matched = declaration.firstMatch(in: text, range: whole).flatMap { Range($0.range, in: text) }.map { String(text[$0]) } ?? text
                let bare = matched.replacingOccurrences(of: "private(set) ", with: "").replacingOccurrences(of: " var ", with: " let ")
                found["\(relative):\(type).\(bare)"] = text
            }
        }
        return found
    }
}
