#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// Grey bars where text will be, while the node is still getting the data ready. It is not an error:
/// the read is asked again, and an error shows only when it keeps failing.
struct SkeletonLines: View {
    var lines = 3
    private static let sentences = [
        "Placeholder text that is as long as a line", "Placeholder text of a shorter line", "Placeholder text of a short one",
    ]

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(0..<lines, id: \.self) { index in
                Text(Self.sentences[index % Self.sentences.count]).font(.subheadline).lineLimit(1)
            }
        }
        .redacted(reason: .placeholder)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Ja.Loading.loading)
    }
}

/// A card of grey bars.
struct SkeletonCard: View {
    var lines = 3

    var body: some View {
        SkeletonLines(lines: lines).card()
    }
}
#endif
