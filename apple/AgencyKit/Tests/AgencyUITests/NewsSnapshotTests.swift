#if canImport(SwiftUI) && os(macOS)
@testable import AgencyModel
import SwiftUI
import Testing
@testable import AgencyUI

/// The banners at phone width, from made-up events.
@MainActor
@Suite("News renderings", .serialized)
struct NewsSnapshotTests {
    private func banner(_ kind: EventKind, _ text: String, count: Int = 1) -> NewsBanner {
        NewsBanner(kind: kind, text: text, count: count, destination: .devices)
    }

    private func expectFits(_ result: Snapshot.Result, _ name: String) {
        #expect(!result.png.isEmpty, "\(name) was not drawn")
        #expect(result.size.width == Snapshot.phoneWidth, "\(name) is \(result.size.width) points wide")
    }

    @Test("one banner, three banners, and a very long text, in light and dark")
    func banners() {
        let one = [banner(.request, "app: 「Which database should the app use in production?」があなたを待っています")]
        let three = [
            banner(.request, "3 件のリクエストがあなたを待っています", count: 3),
            banner(.device, "端末「Tablet」が追加を待っています"),
            banner(.decision, "「Drop support for the old export format?」の決定は断られました"),
        ]
        let long = [banner(.blocked, "app: 「" + String(repeating: "とても長い題名の仕事", count: 12) + "feature-branch-with-a-very-long-name-and-no-spaces-0123456789-abcdefghijklmnopqrstuvwxyz」が止まりました (相談待ち)")]
        for scheme in [ColorScheme.light, .dark] {
            expectFits(Snapshot.capture("news-one", NewsBannerStack(banners: one, onOpen: { _ in }, onClose: {}), scheme: scheme), "one (\(scheme))")
            expectFits(Snapshot.capture("news-three", NewsBannerStack(banners: three, onOpen: { _ in }, onClose: {}), scheme: scheme), "three (\(scheme))")
            expectFits(Snapshot.capture("news-long", NewsBannerStack(banners: long, onOpen: { _ in }, onClose: {}), scheme: scheme), "long (\(scheme))")
        }
    }
}
#endif
