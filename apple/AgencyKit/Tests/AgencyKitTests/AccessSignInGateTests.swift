import Foundation
import Testing
@testable import AgencyKit

@Suite("AccessSignInGate")
struct AccessSignInGateTests {
    private let now = Date(timeIntervalSince1970: 1_800_000_000)

    @Test("the first login page starts a session, and while it runs more pages wait")
    func startThenWait() {
        var gate = AccessSignInGate()
        #expect(gate.next(now: now) == .start)
        #expect(gate.next(now: now) == .wait)
    }

    @Test("a canceled session lets the next login page start another")
    func canceled() {
        var gate = AccessSignInGate()
        _ = gate.next(now: now)
        gate.canceled()
        #expect(gate.next(now: now) == .start)
    }

    @Test("a failed session leaves login pages to the web view for good")
    func failed() {
        var gate = AccessSignInGate()
        _ = gate.next(now: now)
        gate.failed()
        #expect(gate.next(now: now) == .loadInWebView)
        #expect(gate.next(now: now.addingTimeInterval(86_400)) == .loadInWebView)
    }

    @Test("a login page right after a sign-in loads in the web view; later it starts again")
    func repeatWindow() {
        var gate = AccessSignInGate()
        _ = gate.next(now: now)
        gate.signedIn(at: now)
        #expect(gate.next(now: now.addingTimeInterval(1)) == .loadInWebView)
        #expect(gate.next(now: now.addingTimeInterval(AccessSignInGate.repeatWindow - 1)) == .loadInWebView)
        #expect(gate.next(now: now.addingTimeInterval(AccessSignInGate.repeatWindow)) == .start)
    }
}
