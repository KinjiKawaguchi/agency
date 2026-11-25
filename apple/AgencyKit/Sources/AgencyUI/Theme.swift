#if canImport(SwiftUI)
import SwiftUI

/// Spacing and colors shared by the screens. Colors come from the system, so light and dark and the
/// user's accent follow the device.
enum Theme {
    static let gutter: CGFloat = 16
    static let spacing: CGFloat = 12
    static let corner: CGFloat = 12

    static let card = Color.primary.opacity(0.06)
    static let hairline = Color.primary.opacity(0.14)
    static let warning = Color.orange
    static let danger = Color.red
}

extension View {
    /// A text that wraps to the width it is given and never makes its screen wider.
    func wrapping() -> some View {
        fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
    }

    func card(selected: Bool = false) -> some View {
        padding(Theme.spacing)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.card, in: RoundedRectangle(cornerRadius: Theme.corner))
            .overlay(RoundedRectangle(cornerRadius: Theme.corner).stroke(selected ? Color.accentColor : Theme.hairline, lineWidth: selected ? 2 : 1))
    }
}

/// A small label such as a request's kind.
struct Tag: View {
    let text: String
    var color: Color = .accentColor

    var body: some View {
        Text(text)
            .font(.caption.weight(.semibold))
            .padding(.horizontal, 8)
            .padding(.vertical, 2)
            .foregroundStyle(color)
            .background(color.opacity(0.14), in: Capsule())
            .fixedSize()
    }
}
#endif
