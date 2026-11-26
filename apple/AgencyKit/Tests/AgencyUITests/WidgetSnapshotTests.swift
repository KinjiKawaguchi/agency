#if canImport(SwiftUI) && os(macOS)
import AgencyWidget
import AppKit
import SwiftUI
import Testing
@testable import AgencyUI

/// The widgets drawn at their real sizes, offscreen. The rule checked: what a widget draws fits its frame, so no
/// text is cut by the widget's edge. With `AGENCY_SNAPSHOT_DIR` set, each test leaves a PNG to look at.
@MainActor
@Suite("Widget renderings", .serialized)
struct WidgetRenderingTests {
    private struct Size {
        let name: String
        let layout: WidgetLayout
        let width: CGFloat
        let height: CGFloat
        /// The home screen's widgets have a margin; the lock screen's and the inline one have none.
        var padding: CGFloat { layout == .small || layout == .medium ? 11 : 0 }
    }

    // The points of the smallest iPhone's widgets (iPhone SE class): the tightest the system makes them.
    private static let sizes = [
        Size(name: "small", layout: .small, width: 141, height: 141),
        Size(name: "medium", layout: .medium, width: 292, height: 141),
        Size(name: "circular", layout: .circular, width: 76, height: 76),
        Size(name: "rectangular", layout: .rectangular, width: 172, height: 76),
        Size(name: "inline", layout: .inline, width: 250, height: 22),
    ]

    private let now = Date(timeIntervalSince1970: 1_795_521_600)
    private let long = "来年度のクラウド基盤をマネージドサービスへ全面的に移行し、既存の自前運用サーバーを段階的に廃止する計画について承認をお願いします"

    private func request(_ id: String, _ title: String, team: String = "App", kind: WidgetRequest.Kind = .proposal) -> WidgetRequest {
        WidgetRequest(org: "acme", team: "app", request: id, kind: kind, title: title, teamLabel: team)
    }

    private func snapshot(count: Int, titles: [String], written: Date? = nil) -> WidgetSnapshot {
        let requests = titles.enumerated().map { request("r-\($0.offset)", $0.element, kind: [.proposal, .question, .pendingDecision][$0.offset % 3]) }
        return WidgetSnapshot(host: "node.example", writtenAt: written ?? now, waitingCount: count, requests: requests)
    }

    private func draw(_ size: Size, _ snapshot: WidgetSnapshot?, scheme: ColorScheme, at date: Date? = nil) -> (height: CGFloat, png: Data) {
        let content = WidgetContent(snapshot: snapshot, layout: size.layout, now: date ?? now, showsLinks: false)
        // The natural height at the widget's width: if it is taller than the widget, something would be cut.
        let natural = ImageRenderer(content: content.padding(size.padding).frame(width: size.width))
        natural.proposedSize = ProposedViewSize(width: size.width, height: nil)
        let naturalHeight = natural.nsImage?.size.height ?? .infinity
        let framed = content.padding(size.padding).frame(width: size.width, height: size.height)
            .background(scheme == .dark ? Color(white: 0.12) : Color(white: 0.95)).environment(\.colorScheme, scheme)
        let renderer = ImageRenderer(content: framed)
        renderer.scale = 3
        guard let image = renderer.nsImage, let tiff = image.tiffRepresentation, let bitmap = NSBitmapImageRep(data: tiff),
              let png = bitmap.representation(using: .png, properties: [:]) else { return (naturalHeight, Data()) }
        return (naturalHeight, png)
    }

    private func check(_ name: String, _ snapshot: WidgetSnapshot?, sizes: [Size] = WidgetRenderingTests.sizes, at date: Date? = nil, sourceLocation: SourceLocation = #_sourceLocation) {
        for size in sizes {
            for scheme in [ColorScheme.light, .dark] {
                let (height, png) = draw(size, snapshot, scheme: scheme, at: date)
                #expect(!png.isEmpty, "\(name) \(size.name) was not drawn", sourceLocation: sourceLocation)
                #expect(height <= size.height + 0.5, "\(name) \(size.name) needs \(height) points of height, the widget has \(size.height)", sourceLocation: sourceLocation)
                if let directory = Snapshot.outputDirectory, !png.isEmpty {
                    try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
                    try? png.write(to: directory.appendingPathComponent("widget-\(name)-\(size.name)-\(scheme == .dark ? "dark" : "light").png"))
                }
            }
        }
    }

    @Test("three requests waiting: the count and the first request or the first three")
    func waiting() {
        check("waiting", snapshot(count: 7, titles: ["クラウド移行計画の承認", "どのデータベースを使いますか?", "旧形式の廃止を決める"]))
    }

    @Test("a very long title and a long team name do not make a widget taller than it is")
    func longTitles() {
        let requests = (0..<3).map { request("r-\($0)", long, team: "チームの名前がとても長い場合でも収まるか") }
        check("long", WidgetSnapshot(host: "h", writtenAt: now, waitingCount: 12, requests: requests))
    }

    @Test("a count of three digits fits")
    func bigCount() {
        check("count", snapshot(count: 128, titles: ["決めること"]))
    }

    @Test("nothing waiting, and no snapshot yet, say so")
    func nothing() {
        check("empty", snapshot(count: 0, titles: []))
        check("unsynced", nil)
    }

    @Test("an old snapshot says how old it is, and still fits")
    func stale() {
        let written = now.addingTimeInterval(-30 * 3600)
        check("stale", snapshot(count: 2, titles: ["クラウド移行計画の承認", "旧形式の廃止を決める"], written: written))
    }

    @Test("the link of a widget is its first request, and none when there is nothing to open")
    func links() {
        #expect(WidgetContent.link(for: snapshot(count: 2, titles: ["a", "b"]))?.absoluteString == "agency://request?org=acme&team=app&request=r-0")
        #expect(WidgetContent.link(for: snapshot(count: 0, titles: [])) == nil)
        #expect(WidgetContent.link(for: nil) == nil)
    }
}
#endif
