#if canImport(SwiftUI) && os(macOS)
import AgencyAPI
import AgencyModel
import SwiftUI
import Testing
@testable import AgencyUI

/// The card that says whether APNs takes this phone's notices, drawn at phone width: a refusal with a long reason, a refusal with none
/// taken, and a token with nothing to say yet. With `AGENCY_SNAPSHOT_DIR` set, each test leaves a PNG to look at.
@MainActor
@Suite("Push delivery renderings", .serialized)
struct PushDeliverySnapshotTests {
    private func token(taken: String?, failed: String?, reason: String) -> RegisteredPushToken {
        let taken = taken.map { #","lastSucceededAt":"\#($0)""# } ?? ""
        let failed = failed.map { #","lastFailure":{"at":"\#($0)","reason":"\#(reason)"}"# } ?? ""
        let json = #"{"token":"ab12cd34","environment":"production","label":"iPhone","registeredAt":"2026-11-20T09:00:00Z"\#(taken)\#(failed)}"#
        return try! JSONDecoder().decode(RegisteredPushToken.self, from: Data(json.utf8))
    }

    @Test("a refusal with a long reason and a taking before it, and one with none taken")
    func drawn() {
        let long = "APNs refused the provider token (\(UIFixtures.unbrokenID))"
        let states: [(String, PushDelivery)] = [
            ("push-failing", PushDelivery(token: token(taken: "2026-11-26T08:00:00Z", failed: "2026-11-26T09:00:00Z", reason: long))),
            ("push-never", PushDelivery(token: token(taken: nil, failed: "2026-11-26T09:00:00Z", reason: "APNs could not be reached"))),
        ]
        for (name, delivery) in states {
            let lines = try! #require(delivery.lines(in: TimeZone(identifier: "Asia/Tokyo")!))
            for scheme in [ColorScheme.light, .dark] {
                let result = Snapshot.capture(name, PushDeliveryCard(lines: lines), scheme: scheme)
                #expect(!result.png.isEmpty)
                #expect(result.size.width == Snapshot.phoneWidth, "\(name): \(result.size.width) points wide, the screen is \(Snapshot.phoneWidth)")
            }
        }
    }
}
#endif
