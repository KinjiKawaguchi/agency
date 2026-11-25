import Foundation

/// A proposal's document as the screen shows it: a small, safe subset of Markdown. Headings, lists,
/// quotes, code and paragraphs are blocks; the text inside a block keeps its inline marks (bold,
/// italics, code) for the view to style. Raw HTML is never interpreted, and links and images are not
/// followed: the view shows them as text.
public enum MarkdownBlock: Equatable, Sendable {
    case heading(level: Int, text: String)
    case paragraph(String)
    case bullet(String)
    case numbered(number: Int, text: String)
    case quote(String)
    case code(String)

    public static func parse(_ document: String) -> [MarkdownBlock] {
        var blocks: [MarkdownBlock] = []
        var paragraph: [String] = []
        var code: [String]?

        func endParagraph() {
            if !paragraph.isEmpty { blocks.append(.paragraph(paragraph.joined(separator: "\n"))) }
            paragraph = []
        }

        for raw in document.replacingOccurrences(of: "\r\n", with: "\n").split(separator: "\n", omittingEmptySubsequences: false) {
            let line = String(raw)
            if line.hasPrefix("```") {
                if let lines = code {
                    blocks.append(.code(lines.joined(separator: "\n")))
                    code = nil
                } else {
                    endParagraph()
                    code = []
                }
                continue
            }
            if code != nil { code?.append(line); continue }
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.isEmpty { endParagraph(); continue }
            if let block = block(of: trimmed) {
                endParagraph()
                blocks.append(block)
            } else {
                paragraph.append(trimmed)
            }
        }
        if let lines = code { blocks.append(.code(lines.joined(separator: "\n"))) }
        endParagraph()
        return blocks
    }

    private static func block(of line: String) -> MarkdownBlock? {
        let hashes = line.prefix { $0 == "#" }.count
        if (1...6).contains(hashes), line.dropFirst(hashes).first == " " {
            return .heading(level: hashes, text: line.dropFirst(hashes + 1).trimmingCharacters(in: .whitespaces))
        }
        if let marker = line.first, "-*+".contains(marker), line.dropFirst().first == " " {
            return .bullet(String(line.dropFirst(2)).trimmingCharacters(in: .whitespaces))
        }
        if line.hasPrefix(">") {
            return .quote(String(line.dropFirst()).trimmingCharacters(in: .whitespaces))
        }
        let digits = line.prefix { $0.isNumber }
        if !digits.isEmpty, let number = Int(digits), line.dropFirst(digits.count).hasPrefix(". ") {
            return .numbered(number: number, text: String(line.dropFirst(digits.count + 2)).trimmingCharacters(in: .whitespaces))
        }
        return nil
    }
}
