#if canImport(SwiftUI) && os(macOS)
import AppKit
import SwiftUI

/// Renders a view the way it would sit in a phone's column, without launching anything: `ImageRenderer`
/// draws it offscreen. The width is the proposal, and a view that needs more than that makes a wider
/// image, which is how a screen that would scroll sideways shows itself.
@MainActor
enum Snapshot {
    static let phoneWidth: CGFloat = 390

    /// Where PNGs go when `AGENCY_SNAPSHOT_DIR` is set. Nothing is written otherwise.
    static var outputDirectory: URL? {
        ProcessInfo.processInfo.environment["AGENCY_SNAPSHOT_DIR"].map { URL(fileURLWithPath: $0, isDirectory: true) }
    }

    struct Result {
        let size: CGSize
        let png: Data
    }

    static func render<Content: View>(_ content: Content, scheme: ColorScheme = .light, width: CGFloat = phoneWidth) -> Result {
        let framed = content
            .padding(16)
            .frame(width: width, alignment: .topLeading)
            .background(scheme == .dark ? Color(white: 0.08) : Color.white)
            .environment(\.colorScheme, scheme)
        let renderer = ImageRenderer(content: framed)
        renderer.scale = 2
        renderer.proposedSize = ProposedViewSize(width: width, height: nil)
        guard let image = renderer.nsImage, let tiff = image.tiffRepresentation, let bitmap = NSBitmapImageRep(data: tiff),
              let png = bitmap.representation(using: .png, properties: [:])
        else { return Result(size: .zero, png: Data()) }
        return Result(size: image.size, png: png)
    }

    /// Renders, writes the PNG when asked to, and returns the result.
    @discardableResult
    static func capture<Content: View>(_ name: String, _ content: Content, scheme: ColorScheme = .light) -> Result {
        let result = render(content, scheme: scheme)
        if let directory = outputDirectory, !result.png.isEmpty {
            try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            try? result.png.write(to: directory.appendingPathComponent("\(name)-\(scheme == .dark ? "dark" : "light").png"))
        }
        return result
    }
}
#endif
