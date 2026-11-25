#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// Says plainly that this phone has to be added before it can decide, and walks through it: ask, then
/// compare the code, then wait until it is added. Shown above the desk until the phone is ready.
struct PairingCard: View {
    let status: PairingStore.Status
    let isAsking: Bool
    let onAdd: () -> Void
    let onCheck: () -> Void

    var body: some View {
        switch status {
        case .checking, .ready:
            EmptyView()
        case .notRequested:
            card {
                Text(Ja.Device.heading).font(.headline)
                Text(Ja.Device.why).font(.subheadline).wrapping()
                Button(action: onAdd) {
                    Text(isAsking ? Ja.Device.adding : Ja.Device.add).font(.headline).frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(isAsking)
            }
        case .waiting(let code):
            card {
                Text(Ja.Device.waitingHeading).font(.headline)
                Text(code)
                    .font(.system(size: 30, weight: .bold, design: .monospaced))
                    .minimumScaleFactor(0.6)
                    .lineLimit(1)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 6)
                    .accessibilityLabel(code.replacingOccurrences(of: "-", with: " "))
                Text(Ja.Device.waitingBody).font(.subheadline).wrapping()
                Button(action: onCheck) {
                    Text(Ja.Device.check).font(.headline).frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .controlSize(.large)
            }
        case .failed(let reason):
            card {
                Text(Ja.Device.failed(reason)).font(.subheadline).foregroundStyle(Theme.danger).wrapping()
                Button(action: onCheck) {
                    Text(Ja.Desk.reload).font(.headline).frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
            }
        }
    }

    private func card<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: Theme.spacing, content: content).card()
    }
}
#endif
