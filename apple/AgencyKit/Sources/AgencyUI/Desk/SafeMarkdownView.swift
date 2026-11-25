#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// A proposal's document. Only the blocks `MarkdownBlock` knows are drawn; inline marks (bold,
/// italics, code) are kept, and links are shown as plain text so nothing in a document can be tapped.
struct SafeMarkdownView: View {
    let document: String

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(Array(MarkdownBlock.parse(document).enumerated()), id: \.offset) { _, block in
                view(for: block)
            }
        }
    }

    @ViewBuilder
    private func view(for block: MarkdownBlock) -> some View {
        switch block {
        case .heading(let level, let text):
            inline(text).font(level <= 1 ? .title3.bold() : .headline).wrapping()
        case .paragraph(let text):
            inline(text).font(.body).wrapping()
        case .bullet(let text):
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text("•")
                inline(text).wrapping()
            }
        case .numbered(let number, let text):
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text("\(number).").monospacedDigit()
                inline(text).wrapping()
            }
        case .quote(let text):
            inline(text).foregroundStyle(.secondary).padding(.leading, 10)
                .overlay(alignment: .leading) { Rectangle().fill(Theme.hairline).frame(width: 3) }
                .wrapping()
        case .code(let text):
            Text(text).font(.callout.monospaced()).wrapping().card()
        }
    }

    private func inline(_ text: String) -> Text {
        var attributed = (try? AttributedString(markdown: text, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(text)
        for run in attributed.runs where run.link != nil {
            attributed[run.range].link = nil
        }
        return Text(attributed)
    }
}
#endif
