#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// A participant's face: round for a human, a rounded square for an agent, a color that stays with the
/// name, and a ring that shows what they are doing.
struct Face: View {
    enum Size {
        case small, large

        var points: CGFloat { self == .small ? 28 : 46 }
        var font: CGFloat { self == .small ? 11 : 15 }
    }

    let label: String
    let kind: PersonStatus.Kind
    var presence: Presence?
    var size: Size = .large

    private static let hues: [Color] = [
        Color(red: 0.29, green: 0.35, blue: 0.82), Color(red: 0.04, green: 0.49, blue: 0.44),
        Color(red: 0.60, green: 0.37, blue: 0.02), Color(red: 0.52, green: 0.27, blue: 0.74),
        Color(red: 0.13, green: 0.45, blue: 0.68), Color(red: 0.69, green: 0.26, blue: 0.42),
        Color(red: 0.30, green: 0.45, blue: 0.14), Color(red: 0.44, green: 0.37, blue: 0.30),
    ]

    var body: some View {
        Text(Identity.initials(of: label))
            .font(.system(size: size.font, weight: .bold))
            .foregroundStyle(.white)
            .frame(width: size.points, height: size.points)
            .background(Self.hues[Identity.hue(of: label) - 1], in: shape)
            .overlay { ring }
            .opacity(presence == .away ? 0.45 : 1)
            .padding(3)
            .accessibilityHidden(true)
    }

    private var shape: RoundedRectangle {
        RoundedRectangle(cornerRadius: kind == .human ? size.points / 2 : size.points * 0.28)
    }

    @ViewBuilder
    private var ring: some View {
        switch presence {
        case .needsYou: shape.inset(by: -3).stroke(Color.accentColor, lineWidth: 2)
        case .working: shape.inset(by: -3).stroke(Color.green, lineWidth: 2)
        case .waiting: shape.inset(by: -3).stroke(Theme.warning, lineWidth: 2)
        case .away: shape.inset(by: -3).stroke(Color.secondary, style: StrokeStyle(lineWidth: 1.5, dash: [3, 2]))
        case .idle, nil: EmptyView()
        }
    }
}
#endif
