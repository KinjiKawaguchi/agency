#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// Whether APNs takes the notices the node sends this phone. A notice that APNs did not take is a warning with its reason, as the node
/// gives it; that none was taken yet is quiet text. It says nothing at all when APNs takes them.
struct PushDeliveryCard: View {
    let lines: [String]
    let isFailing: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Label {
                Text(Ja.Devices.pushTitle).font(.headline)
            } icon: {
                Image(systemName: isFailing ? "exclamationmark.triangle.fill" : "bell").foregroundStyle(isFailing ? Theme.warning : .secondary)
            }
            ForEach(Array(lines.enumerated()), id: \.offset) { _, line in
                Text(line).font(.subheadline).foregroundStyle(isFailing ? .primary : .secondary).wrapping()
            }
        }
        .card()
        .accessibilityElement(children: .combine)
    }
}
#endif
