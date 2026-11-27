import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

@Suite("Standing")
struct StandingTests {
    private func me(_ extra: String = "") throws -> Me {
        let json = #"{"account":{"id":"a","label":"A","fingerprint":"f"},"funding":[],"orgs":[]\#(extra)}"#
        return try JSONDecoder().decode(Me.self, from: Data(json.utf8))
    }

    @Test("a member the node hosts is a guest, and anyone else is a member")
    func fromMe() throws {
        #expect(Standing(me: try me()) == .member)
        #expect(Standing(me: try me(#","guest":null"#)) == .member)
        #expect(Standing(me: try me(#","guest":{"member":"erin","org":"acme"}"#)) == .guest(GuestRef(member: "erin", org: "acme")))
        #expect(Standing(me: try me(#","guest":"odd""#)).isGuest, "fail closed")
        #expect(!Standing.member.isGuest && !Standing.unknown.isGuest && !Standing.loading.isGuest)
    }

    @Test("a guest has the desk and the devices; so has a viewer not told yet; anyone else has every screen, and a failed read counts as anyone else")
    func tabs() {
        let guest = Standing.guest(GuestRef(member: nil, org: nil))
        #expect(AppTab.tabs(for: guest) == [.desk, .devices])
        #expect(AppTab.tabs(for: .loading) == [.desk, .devices], "the owner's screens are not offered for a moment, as on the web")
        #expect(AppTab.tabs(for: .member) == AppTab.allCases)
        #expect(AppTab.tabs(for: .unknown) == AppTab.allCases, "a failed read: the screens show their own failures")
    }
}

@MainActor
@Suite("Shell router and standing")
struct RouterStandingTests {
    private let guest = Standing.guest(GuestRef(member: "erin", org: "acme"))

    @Test("a guest is taken from a screen they do not have to the desk, and cannot open one")
    func guestCannotOpenOwnerScreens() {
        let router = ShellRouter(standing: .member)
        router.tab = .results
        #expect(router.tab == .results)
        router.standing = guest
        #expect(router.tab == .desk, "when the node says the viewer is a guest, an open owner screen closes")
        router.tab = .organization
        #expect(router.tab == .desk)
        router.tab = .devices
        #expect(router.tab == .devices)
        router.tab = .overview
        #expect(router.tab == .desk)
        #expect(router.availableTabs == [.desk, .devices])
    }

    @Test("a tapped banner or notification that leads to an owner screen leads a guest to the desk")
    func leads() {
        let router = ShellRouter()
        router.standing = guest
        router.open(NewsBanner(kind: .request, text: "x", count: 1, destination: .people))
        #expect(router.tab == .desk)
        router.open(NewsBanner(kind: .device, text: "x", count: 1, destination: .devices))
        #expect(router.tab == .devices)
        let member = ShellRouter(standing: .member)
        member.open(NewsBanner(kind: .request, text: "x", count: 1, destination: .people))
        #expect(member.tab == .organization && member.organizationPage == .people)
    }

    @Test("the screens come back when the node says the viewer is not a guest, and the open screen is not changed by it")
    func ownerKeepsTheirScreen() {
        let router = ShellRouter()
        router.tab = .organization
        #expect(router.tab == .desk, "before the node has said, the owner's screens are not offered")
        router.standing = .member
        router.tab = .organization
        #expect(router.tab == .organization)
        router.standing = .unknown
        #expect(router.tab == .organization)
    }
}
