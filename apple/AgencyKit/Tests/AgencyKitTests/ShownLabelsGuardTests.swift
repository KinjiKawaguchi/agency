import Foundation
import Testing

/// A label the node gives is shown without the characters that can reorder it (`NodeLabel`, applied when it is decoded). This reads the
/// wire types and fails for a field that is named like a label and is not decoded that way, so that a new one is not forgotten: any
/// `label`, `name`, `member`, `device`, `what`, `from`, `to`, `choiceLabel`, `reason`, `title`, `text`, `context`, `document`, `detail`, `status`, `note`, `summary` or `message` that is a `String` must be `@Shown`, `@ShownIfPresent`, `@ShownText` or `@ShownTextIfPresent`,
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
        "AgencyKit/Sources/AgencyAPI/Wire/Instant.swift:Instant.public let text: String":
            "a time as the node wrote it: it is read as a time, and an answer with one that cannot be read is refused",
        "AgencyKit/Sources/AgencyAPI/Wire/Metrics.swift:LatencySeries.public let status: String":
            "the class of a status (`2xx`) or the outcome of a call (`ok`, `timeout`) that the node writes itself",
        "AgencyKit/Sources/AgencyAPI/Wire/Outcomes.swift:ObservationInput.public let note: String?":
            "what the owner types and the app sends: not something the node gives",
        "AgencyKit/Sources/AgencyAPI/Wire/RegisteredPushToken.swift:ReportedTime.public let text: String":
            "shown in its initializer (`NodeLabel.shown`), where the time is read from what was written",
        "AgencyKit/Sources/AgencyAPI/Wire/TeamView.swift:Request.public let from: String":
            "the team runtime's name, which finds the person who has it (`Team.person(named:)`), so it is kept as written; a screen draws `shownFrom` (`DeskItem.drafterName`)",
        "AgencyKit/Sources/AgencyAPI/Wire/TeamView.swift:Request.public let to: String?":
            "the team runtime's name, kept as written to find the person; a screen draws `shownTo` (`DeskItem.deciderName`)",
        "AgencyKit/Sources/AgencyAPI/Wire/TeamView.swift:ThreadMessage.public let from: String":
            "the team runtime's name of who wrote a message, kept as written; a screen draws `shownFrom`",
        "AgencyKit/Sources/AgencyAPI/Wire/Me.swift:GuestRef.public let member: String?":
            "the id of the hosted member, which the app reads to know the viewer is a guest and never shows",
        "AgencyKit/Sources/AgencyAPI/Wire/Devices.swift:DeviceRequest.public let rawLabel: String":
            "the raw twin of a shown field, for what is signed and sent back: never drawn (see `sentBack`)",
        "AgencyKit/Sources/AgencyAPI/Wire/TeamView.swift:Request.public let rawTitle: String":
            "the raw twin of a shown field, for what is sent back with a decision: never drawn (see `sentBack`)",
        "AgencyKit/Sources/AgencyAPI/Wire/TeamView.swift:RequestOption.public let rawText: String":
            "the raw twin of a shown field, for what is sent back with a decision: never drawn (see `sentBack`)",
        "AgencyKit/Sources/AgencyAPI/Wire/Outcomes.swift:AimReading.public let epicTitle: String?":
            "the epic's title: its reader shows it with `NodeLabel.shown` (`AimReading.init(from:)`)",
        "AgencyKit/Sources/AgencyAPI/Wire/Effort.swift:Effort.public let from: String": "a calendar day, the period's first: never a label",
        "AgencyKit/Sources/AgencyAPI/Wire/Effort.swift:TeamEffort.public let from: String": "a calendar day, the period's first: never a label",
        "AgencyKit/Sources/AgencyAPI/Wire/Evaluation.swift:Evaluation.public let from: String": "a calendar day, the period's first: never a label",
        "AgencyKit/Sources/AgencyAPI/Wire/Evaluation.swift:TeamEvaluation.public let from: String": "a calendar day, the period's first: never a label",
        "AgencyKit/Sources/AgencyAPI/Wire/Outcomes.swift:Outcomes.public let from: String": "a calendar day, the period's first: never a label",
    ]

    /// What the app sends back to the node or signs is the value the node gave, never the shown one: a display transform must not change
    /// what is signed, or the signature covers bytes nobody sent. A field that is shown at decode and sent back has a twin that is the raw
    /// value (`$field` of the wrapper), and the sending reads the twin. Anything new that is sent back is named here with where.
    /// Strings inside the cases of an enum the node's words are read into (`case refused(message: String)`) are text too, and are shown where
    /// the enum is decoded or made: named here with where, since a `let` or `var` is all the scan above sees.
    private static let allowedPayloads: [String: String] = [
        "AgencyKit/Sources/AgencyAPI/Wire/Outcomes.swift:KpiObservation.case connector.name":
            "the connector's name, shown where the source is decoded (`NodeLabel.shown`)",
        "AgencyKit/Sources/AgencyAPI/Wire/Decisions.swift:DecisionState.case queued.lastError":
            "shown where `DecisionState` is decoded (`NodeLabel.shownText`)",
        "AgencyKit/Sources/AgencyAPI/Wire/Decisions.swift:DecisionState.case refused.message":
            "shown where `DecisionState` is decoded (`NodeLabel.shownText`)",
        "AgencyKit/Sources/AgencyAPI/Wire/TeamSnapshot.swift:TeamSnapshot.case replica.error":
            "shown where the source is decoded (`NodeLabel.shownText`)",
        "AgencyKit/Sources/AgencyAPI/NodeError.swift:NodeError.case http.message":
            "shown where the error is made from a refusal (`NodeClient.refusalText`); the other places that make one pass the app's own words",
    ]

    private static let sentBack: [String: (twin: String, why: String)] = [
        "AgencyKit/Sources/AgencyAPI/Wire/Devices.swift:DeviceRequest.label":
            ("rawLabel", "the label of the device that asks to be added is signed in the change that adds it (`DeviceChangeSender`)"),
        "AgencyKit/Sources/AgencyAPI/Wire/TeamView.swift:Request.title":
            ("rawTitle", "the request's title is sent with a decision for the node's decision log (`PreparedDecision.title`)"),
        "AgencyKit/Sources/AgencyAPI/Wire/TeamView.swift:RequestOption.text":
            ("rawText", "the chosen option's text is sent with a decision for the decision log (`PreparedDecision.choiceLabel`)"),
    ]

    @Test("a field that is sent back to the node has a twin that is what the node wrote, and the sending reads it")
    func sentBackIsRaw() throws {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 { url.deleteLastPathComponent() }
        for (field, entry) in Self.sentBack {
            let parts = field.split(separator: ":", maxSplits: 1).map(String.init)
            let source = try String(contentsOf: url.appendingPathComponent(parts[0]), encoding: .utf8)
            let name = String(parts[1].split(separator: ".").last ?? "")
            #expect(source.contains("public var \(entry.twin): String { $\(name) }"), Comment(rawValue: "\(field): \(entry.why)"))
        }
        let sender = try String(contentsOf: url.appendingPathComponent("AgencyKit/Sources/AgencyModel/Devices/DeviceChangeSender.swift"), encoding: .utf8)
        let reply = try String(contentsOf: url.appendingPathComponent("AgencyKit/Sources/AgencyModel/Desk/DeskReply.swift"), encoding: .utf8)
        #expect(sender.contains("label: request.rawLabel") && !sender.contains("label: request.label"))
        #expect(reply.contains("item.request.rawTitle") && reply.contains("$0.rawText") && !reply.contains("item.request.title"))
    }

    /// The names that read like text a person or an agent wrote, and not like an id or a number.
    private static let nameAlternatives = #"label|name|member|device|what|from|to|reason|title|text|context|document|detail|status|note|summary|message|error|\w+Title|\w+Label|\w+Text|\w+Message|\w+Error"#

    private static let declaration = try! NSRegularExpression(
        pattern: #"public\s+(?:private\(set\)\s+)?(?:let|var)\s+("# + nameAlternatives + #")\s*:\s*String\??(?![\w.])"#
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

    @Test("a string inside a case of an enum of the node's words is shown where it is decoded, or is named here with where")
    func everyPayload() throws {
        let found = try Self.payloads()
        let unnamed = found.filter { Self.allowedPayloads[$0] == nil }.sorted()
        let stale = Self.allowedPayloads.keys.filter { !found.contains($0) }.sorted()
        #expect(unnamed.isEmpty && stale.isEmpty, Comment(rawValue: "not named: \(unnamed); named but gone: \(stale)"))
    }

    @Test("the scan of cases sees a string payload named like text, and not one that is not")
    func payloadScanSees() {
        #expect(Self.payloadFields(in: "case refused(message: String)") == ["message"])
        #expect(Self.payloadFields(in: "case queued(attempts: Int, nextAt: Instant, lastError: String?)") == ["lastError"])
        #expect(Self.payloadFields(in: "case http(status: Int, message: String?)") == ["message"])
        #expect(Self.payloadFields(in: "case person(String)").isEmpty && Self.payloadFields(in: "case replica(fetchedAt: Instant?, triedAt: Instant)").isEmpty)
        #expect(Self.payloadFields(in: "case unknown(String)").isEmpty && Self.payloadFields(in: "let message: String").isEmpty)
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

    private static let payloadName = #"\b("# + nameAlternatives + #")\s*:\s*String\??"#

    /// The names of the string payloads of a `case` that read like text.
    static func payloadFields(in line: String) -> [String] {
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        guard trimmed.hasPrefix("case "), trimmed.contains("("), let regex = try? NSRegularExpression(pattern: payloadName) else { return [] }
        let whole = NSRange(trimmed.startIndex..., in: trimmed)
        return regex.matches(in: trimmed, range: whole).compactMap { Range($0.range(at: 1), in: trimmed).map { String(trimmed[$0]) } }
    }

    /// `file:Type.case name.field` for each such payload in the wire types and in `NodeError`.
    private static func payloads() throws -> Set<String> {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 { url.deleteLastPathComponent() }
        let api = url.appendingPathComponent("AgencyKit/Sources/AgencyAPI")
        var found: Set<String> = []
        let files = try #require(FileManager.default.enumerator(at: api, includingPropertiesForKeys: nil))
        for case let file as URL in files where file.pathExtension == "swift" && (file.path.contains("/Wire/") || file.lastPathComponent == "NodeError.swift") {
            let relative = String(file.path.dropFirst(url.path.count + 1))
            var type = ""
            for line in try String(contentsOf: file, encoding: .utf8).split(separator: "\n") {
                let raw = String(line)
                if let match = typeDeclaration.firstMatch(in: raw, range: NSRange(raw.startIndex..., in: raw)), let name = Range(match.range(at: 1), in: raw) { type = String(raw[name]) }
                let trimmed = raw.trimmingCharacters(in: .whitespaces)
                let caseName = trimmed.dropFirst(5).prefix { $0 != "(" }.trimmingCharacters(in: .whitespaces)
                for field in payloadFields(in: raw) { found.insert("\(relative):\(type).case \(caseName).\(field)") }
            }
        }
        return found
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
