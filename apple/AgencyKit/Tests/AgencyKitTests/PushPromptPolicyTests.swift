import Foundation
import Testing
@testable import AgencyKit

@Suite("PushPromptPolicy")
struct PushPromptPolicyTests {
    private let now = Date(timeIntervalSince1970: 1_800_000_000)
    private let day: TimeInterval = 24 * 60 * 60

    private func next(_ authorization: NotificationAuthorization, signedIn: Bool = true, deferred: Date? = nil) -> PushStep {
        PushPromptPolicy.next(authorization: authorization, isSignedIn: signedIn, lastDeferredAt: deferred, now: now)
    }

    @Test("it does not ask before the owner has signed in")
    func waitsForSignIn() {
        #expect(next(.notDetermined, signedIn: false) == .nothing)
        #expect(next(.notDetermined, signedIn: true) == .askForPermission)
    }

    @Test("after Not now it asks again only once a week has passed")
    func waitsAfterDeferral() {
        #expect(next(.notDetermined, deferred: now.addingTimeInterval(-6 * day)) == .nothing)
        #expect(next(.notDetermined, deferred: now.addingTimeInterval(-8 * day)) == .askForPermission)
    }

    @Test("a clock set back after Not now does not ask early")
    func clockSetBack() {
        #expect(next(.notDetermined, deferred: now.addingTimeInterval(day)) == .nothing)
    }

    @Test("a granted permission registers, signed in or not, and a refusal is never asked again")
    func grantedAndDenied() {
        #expect(next(.authorized, signedIn: false) == .register)
        #expect(next(.authorized, deferred: now) == .register)
        #expect(next(.denied) == .nothing)
        #expect(next(.denied, deferred: now.addingTimeInterval(-30 * day)) == .nothing)
    }
}
