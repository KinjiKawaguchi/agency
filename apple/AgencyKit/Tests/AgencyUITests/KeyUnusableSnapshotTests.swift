#if canImport(SwiftUI) && os(macOS)
import AgencyModel
import SwiftUI
import Testing
@testable import AgencyUI

/// The screen for a key that is stored and cannot be used, at phone width: nothing is wider than the screen, even with a long reason. With
/// `AGENCY_SNAPSHOT_DIR` set, each test leaves a PNG to look at.
@MainActor
@Suite("Key unusable renderings", .serialized)
struct KeyUnusableSnapshotTests {
    @Test("the screen with a short reason and with a long, unbroken one, light and dark")
    func drawn() {
        let reasons = ["the phone is locked (-25308)", "Keychain error (-25293) \(UIFixtures.unbrokenID) and what the system said after it, at some length, so that it wraps"]
        for (index, reason) in reasons.enumerated() {
            for scheme in [ColorScheme.light, .dark] {
                let screen = KeyUnusableContent(problem: KeyProblem(reason: reason, isTransient: index == 0), onTryAgain: {}, confirmsNew: .constant(false)).padding(Theme.gutter)
                let result = Snapshot.capture("key-unusable-\(index)", screen, scheme: scheme)
                #expect(!result.png.isEmpty)
                #expect(result.size.width == Snapshot.phoneWidth, "key-unusable-\(index): \(result.size.width) points wide, the screen is \(Snapshot.phoneWidth)")
            }
        }
    }
}
#endif
