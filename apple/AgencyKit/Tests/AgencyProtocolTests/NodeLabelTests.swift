@testable import AgencyProtocol
import Foundation
import Testing

@Suite("Labels from the node are shown without what can reorder them")
struct NodeLabelTests {
    private func scalar(_ value: UInt32) -> String { String(Unicode.Scalar(value)!) }

    @Test("the control characters, the bidirectional controls, the zero-width space, the word joiner and the byte order mark are shown as ?")
    func hidden() {
        var shown: [UInt32] = [0x00, 0x09, 0x0A, 0x0D, 0x1F, 0x7F, 0x80, 0x9F, 0x061C, 0x200B, 0x200E, 0x200F, 0x2060, 0xFEFF]
        shown += Array(0x202A...0x202E) + Array(0x2066...0x2069)
        for value in shown {
            #expect(NodeLabel.shown("a" + scalar(value) + "b") == "a?b", Comment(rawValue: "U+\(String(value, radix: 16, uppercase: true))"))
        }
    }

    @Test("a right-to-left override is shown, so that it cannot reorder what follows it")
    func override() {
        #expect(NodeLabel.shown("Erin\u{202E}) 代理で答える") == "Erin?) 代理で答える")
        #expect(NodeLabel.shown("\u{2066}x\u{2069}") == "?x?")
        #expect(NodeLabel.shown("zero\u{200B}width") == "zero?width")
    }

    @Test("what is part of how a script or an emoji is written stays: the joiner and non-joiner, tag characters, text in any script")
    func kept() {
        let family = "👨\u{200D}👩\u{200D}👧"
        let flag = "🏴\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}"
        let persian = "می\u{200C}خواهم"
        for text in [family, flag, persian, "Acme", "営業部 (Sales)", "O’Brien — “quoted”", "a b", "\u{00A0}", "\u{2028}", "é"] {
            #expect(NodeLabel.shown(text) == text, Comment(rawValue: text))
        }
    }

    @Test("a label with none of them is returned as it is, and every one of several is replaced")
    func several() {
        #expect(NodeLabel.shown("") == "")
        #expect(NodeLabel.shown("\u{202E}\u{200B}\n") == "???")
        #expect(NodeLabel.isHidden("\u{202E}") && !NodeLabel.isHidden("\u{200D}") && !NodeLabel.isHidden("a"))
    }

    @Test("the property wrapper shows a decoded label and one that is set, and decodes everything else as it was written")
    func wrapper() throws {
        struct Named: Decodable, Equatable {
            @Shown var label: String
            let id: String
        }
        let named = try JSONDecoder().decode(Named.self, from: Data(#"{"id":"a\u202Eb","label":"Erin\u202E (x)\u200B"}"#.utf8))
        #expect(named.label == "Erin? (x)?", "the label is shown")
        #expect(named.id == "a\u{202E}b", "only what is a label is changed")
        var set = Shown(wrappedValue: "x\u{FEFF}y")
        #expect(set.wrappedValue == "x?y")
        set.wrappedValue = "\u{202D}"
        #expect(set.wrappedValue == "?")
    }
}
