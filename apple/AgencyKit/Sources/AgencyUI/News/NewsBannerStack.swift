#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// The banners for what is new, at the top of the screen while the app is open. Tapping one goes where
/// it leads; the cross closes them all. Nothing here scrolls sideways: a long text wraps.
public struct NewsBannerStack: View {
    private let banners: [NewsBanner]
    private let onOpen: (NewsBanner) -> Void
    private let onClose: () -> Void

    public init(banners: [NewsBanner], onOpen: @escaping (NewsBanner) -> Void, onClose: @escaping () -> Void) {
        self.banners = banners
        self.onOpen = onOpen
        self.onClose = onClose
    }

    public var body: some View {
        if !banners.isEmpty {
            HStack(alignment: .top, spacing: 8) {
                VStack(spacing: 8) {
                    ForEach(banners) { banner in
                        Button { onOpen(banner) } label: { row(banner) }
                            .buttonStyle(.plain)
                    }
                }
                Button(action: onClose) {
                    Image(systemName: "xmark.circle.fill").font(.title3).foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(Ja.News.close)
            }
            .padding(Theme.spacing)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: Theme.corner))
            .overlay(RoundedRectangle(cornerRadius: Theme.corner).stroke(Theme.hairline))
            .shadow(color: .black.opacity(0.15), radius: 8, y: 2)
            .padding(.horizontal, Theme.gutter)
            .transition(.move(edge: .top).combined(with: .opacity))
        }
    }

    private func row(_ banner: NewsBanner) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Image(systemName: symbol(of: banner.kind)).foregroundStyle(Color.accentColor)
            Text(banner.text)
                .font(.subheadline.weight(.semibold))
                .multilineTextAlignment(.leading)
                .lineLimit(3)
                .wrapping()
        }
        .contentShape(Rectangle())
    }

    private func symbol(of kind: EventKind) -> String {
        switch kind {
        case .request: "tray.full"
        case .participant: "person.fill.questionmark"
        case .blocked: "exclamationmark.octagon"
        case .device: "iphone"
        case .decision: "checkmark.seal"
        }
    }
}
#endif
