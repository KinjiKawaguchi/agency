#if canImport(SwiftUI) && os(macOS)
import AgencyAPI
import AgencyModel
import SwiftUI
import Testing
@testable import AgencyUI

/// The started-recovery item drawn at phone width. The rule checked is the owner's: nothing is wider than the screen, even
/// with a long org name and a long fingerprint. With `AGENCY_SNAPSHOT_DIR` set, each test leaves a PNG to look at.
@MainActor
@Suite("Recovery renderings", .serialized)
struct RecoverySnapshotTests {
    private func notice(label: String, extra: String) -> RecoveryNotice {
        let json = #"{"id":"acme","label":"\#(label)","isRoot":true,"teams":[],"recovery":{\#(extra)}}"#
        let org = try! JSONDecoder().decode(OrgRef.self, from: Data(json.utf8))
        return RecoveryNotice(org: org, recovery: org.recovery!)
    }

    @Test("the item with a time, a new root key and a replaced recovery key, and with a long name and nothing else")
    func drawn() {
        let tokyo = TimeZone(identifier: "Asia/Tokyo")!
        let full = notice(label: "アクミ", extra: #""takesEffectAt":"2026-11-29T09:30:00Z","newRootKey":"4f9a1c0be2d7a35c","replacesRecoveryKey":true"#)
        let bare = notice(label: UIFixtures.unbrokenID, extra: "")
        for scheme in [ColorScheme.light, .dark] {
            let view = VStack(alignment: .leading, spacing: 12) { RecoveryBanner(notice: full, timeZone: tokyo); RecoveryBanner(notice: bare, timeZone: tokyo) }
            let result = Snapshot.capture("recovery", view, scheme: scheme)
            #expect(!result.png.isEmpty)
            #expect(result.size.width == Snapshot.phoneWidth, "\(result.size.width) points wide, the screen is \(Snapshot.phoneWidth)")
        }
    }
}
#endif
