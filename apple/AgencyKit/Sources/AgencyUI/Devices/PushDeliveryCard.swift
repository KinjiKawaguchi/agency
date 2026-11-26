#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// Whether APNs takes the notices the node sends this phone: drawn only when a notice was refused, with the reason as the node gives it.
struct PushDeliveryCard: View {
    let lines: [String]

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Label {
                Text(Ja.Devices.pushTitle).font(.headline)
            } icon: {
                Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(Theme.warning)
            }
            ForEach(Array(lines.enumerated()), id: \.offset) { _, line in
                Text(line).font(.subheadline).wrapping()
            }
        }
        .card()
        .accessibilityElement(children: .combine)
    }
}
#endif
