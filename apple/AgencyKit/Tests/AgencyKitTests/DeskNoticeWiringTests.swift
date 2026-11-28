import Foundation
import Testing

/// What `DeskNotice` and `NodeCompatibility` decide reaches the desk only through `ShellModel.notice`, which is in the app and which no test of the
/// package runs: a comparison that is made and never handed to the desk says nothing to anyone. This reads the source for the call.
@Suite("The desk's notice is the one the versions give")
struct DeskNoticeWiringTests {
    private static func appleRoot(from file: String = #filePath) -> URL {
        // Tests/AgencyKitTests/DeskNoticeWiringTests.swift: four levels up is `apple/`.
        var url = URL(fileURLWithPath: file)
        for _ in 0..<4 { url.deleteLastPathComponent() }
        return url
    }

    /// The file with every run of white space as one space, so that a call split over lines reads as one.
    private static func source(_ path: String) throws -> String {
        let text = try String(contentsOf: appleRoot().appendingPathComponent(path), encoding: .utf8)
        return text.split(whereSeparator: \.isWhitespace).joined(separator: " ")
    }

    @Test("the shell's notice is made by DeskNotice from the session's versions and the viewer's standing")
    func shellModel() throws {
        let text = try Self.source("Shared/Shell/ShellModel.swift")
        #expect(text.contains("DeskNotice.first( compatibility: session.compatibility, isGuest: session.desk.standing.isGuest,"), "ShellModel.notice must hand the comparison and the standing to the desk")
    }

    @Test("the session compares what the node says of itself, and the shell asks it to when the desk was read or failed")
    func sessionAndShell() throws {
        let session = try Self.source("Shared/Native/NativeSession.swift")
        #expect(session.contains("NodeCompatibility.judge(") && session.contains("compatibility = "))
        let shell = try Self.source("Shared/Shell/ShellModel.swift")
        #expect(shell.components(separatedBy: "checkNodeVersion()").count - 1 >= 3, "asked for in the loaded and the failed case, and defined once")
    }
}
