#if canImport(SwiftUI)
import SwiftUI

/// A name or title that is cut after a few lines, and shows all of it when tapped.
struct ExpandableTitle: View {
    let text: String
    var font: Font = .headline
    var lineLimit = 2
    @State private var isExpanded = false

    var body: some View {
        Text(text)
            .font(font)
            .multilineTextAlignment(.leading)
            .lineLimit(isExpanded ? nil : lineLimit)
            .wrapping()
            .contentShape(Rectangle())
            .onTapGesture { isExpanded.toggle() }
            .accessibilityAddTraits(.isButton)
    }
}
#endif
