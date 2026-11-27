#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// What the person sees when this device's key is stored and cannot be used now: plainly, with the reason the system gave, and two ways on.
/// Nothing was replaced and nothing is replaced by this screen: trying again reads the stored key again, and making a new key is a choice
/// the person confirms, after which the device is added again.
public struct KeyUnusableScreen: View {
    private let problem: KeyProblem
    private let onTryAgain: () -> Void
    private let onMakeNew: () -> Void
    @State private var confirmsNew = false

    public init(problem: KeyProblem, onTryAgain: @escaping () -> Void, onMakeNew: @escaping () -> Void) {
        self.problem = problem
        self.onTryAgain = onTryAgain
        self.onMakeNew = onMakeNew
    }

    public var body: some View {
        ScrollView {
            KeyUnusableContent(problem: problem, onTryAgain: onTryAgain, confirmsNew: $confirmsNew)
        }
        .confirmationDialog(Ja.Device.reset, isPresented: $confirmsNew, titleVisibility: .visible) {
            Button(Ja.Device.resetConfirm, role: .destructive, action: onMakeNew)
        } message: {
            Text(Ja.Key.Unusable.makeExplain)
        }
    }
}

/// The screen's text and buttons, without the scrolling, so that a rendering can draw it.
struct KeyUnusableContent: View {
    let problem: KeyProblem
    let onTryAgain: () -> Void
    @Binding var confirmsNew: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            Image(systemName: "lock.slash").font(.largeTitle).foregroundStyle(.secondary).accessibilityHidden(true)
            Text(Ja.Key.Unusable.title).font(.title2.weight(.semibold)).wrapping()
            Text(Ja.Key.Unusable.explain).font(.body).wrapping()
            Text(Ja.Key.Unusable.reason(problem.reason)).font(.system(.footnote, design: .monospaced)).foregroundStyle(.secondary).wrapping()
            Button(action: onTryAgain) {
                Text(Ja.Key.Unusable.tryAgain).font(.headline).frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            Button(role: .destructive) { confirmsNew = true } label: {
                Text(Ja.Device.reset).font(.headline).frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .controlSize(.large)
        }
        .padding(Theme.gutter)
        .frame(maxWidth: 560, alignment: .leading)
        .frame(maxWidth: .infinity)
    }
}
#endif
