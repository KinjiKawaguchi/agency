#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// The numbers of a KPI over its period as dots on their days, joined by a line, with nothing drawn for a day with no
/// number. Its height is small and it carries no axis: the card says the number, and this says only the course.
struct TrendView: View {
    let layout: TrendLayout
    var tint: Color = .accentColor

    var body: some View {
        Canvas { context, size in
            let inset: CGFloat = 5
            let width = max(size.width - 2 * inset, 1)
            let height = max(size.height - 2 * inset, 1)
            let places = layout.spots.map { CGPoint(x: inset + $0.x * width, y: inset + (1 - $0.y) * height) }
            context.stroke(
                Path { path in
                    path.move(to: CGPoint(x: 0, y: size.height - 0.5))
                    path.addLine(to: CGPoint(x: size.width, y: size.height - 0.5))
                },
                with: .color(Theme.hairline), lineWidth: 1
            )
            if places.count > 1 {
                var line = Path()
                line.move(to: places[0])
                for place in places.dropFirst() { line.addLine(to: place) }
                context.stroke(line, with: .color(tint.opacity(0.6)), lineWidth: 2)
            }
            for place in places {
                context.fill(Path(ellipseIn: CGRect(x: place.x - 3, y: place.y - 3, width: 6, height: 6)), with: .color(tint))
            }
        }
        .frame(height: 44)
        .accessibilityHidden(true)
    }
}
#endif
