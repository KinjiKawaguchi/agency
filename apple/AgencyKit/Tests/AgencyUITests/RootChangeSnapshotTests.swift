#if canImport(SwiftUI) && os(macOS)
import AgencyAPI
import AgencyModel
import AgencyProtocol
import SwiftUI
import Testing
@testable import AgencyUI

/// The item for what waits for the root key's signature, drawn at phone width. The rule checked is the owner's: nothing is wider
/// than the screen, even with a long member name, an unbroken id and several changes. With `AGENCY_SNAPSHOT_DIR` set, each
/// test leaves a PNG to look at.
@MainActor
@Suite("Root change renderings", .serialized)
struct RootChangeSnapshotTests {
    private func notice(_ changes: [String]) -> RootChangeNotice {
        let json = #"{"account":{"id":"alice","label":"A","fingerprint":"f"},"funding":[],"orgs":[{"id":"acme","label":"アクミ","isRoot":true,"teams":[]}],"rootChanges":[\#(changes.joined(separator: ","))]}"#
        return RootChangeNotice(me: try! JSONDecoder().decode(Me.self, from: Data(json.utf8)))!
    }

    private func change(_ fields: String, org: String = "acme") -> String {
        #"{"id":"c\#(fields.count)","org":"\#(org)","receivedAt":"2026-11-26T09:00:00Z"\#(fields)}"#
    }

    @Test("one change, several kinds with a long name, and the node's own words for a kind the app does not know")
    func drawn() {
        let now = Timestamp.date(from: "2026-11-26T12:00:00Z")!
        let one = notice([change(#","kind":"addMember","member":"carol","label":"Carol""#)])
        let many = notice([
            change(#","kind":"addDevice","member":"bob","label":"iPhone 17 Pro, the one in the drawer""#),
            change(#","kind":"removeDevice","member":"bob","device":"\#(UIFixtures.unbrokenID)""#),
            change(#","what":"add member carol (Carol)","kind":"somethingNew""#, org: UIFixtures.unbrokenID),
        ])
        for scheme in [ColorScheme.light, .dark] {
            let view = VStack(alignment: .leading, spacing: 12) { RootChangeBanner(notice: one, now: now); RootChangeBanner(notice: many, now: now) }
            let result = Snapshot.capture("root-changes", view, scheme: scheme)
            #expect(!result.png.isEmpty)
            #expect(result.size.width == Snapshot.phoneWidth, "\(result.size.width) points wide, the screen is \(Snapshot.phoneWidth)")
        }
    }
}
#endif
