import AgencyKit
@testable import AgencyModel
import Foundation
import Testing

@MainActor
@Suite("ShellRouter")
struct ShellRouterTests {
    private func banner(_ kind: EventKind, _ destination: NewsDestination) -> NewsBanner {
        NewsBanner(kind: kind, text: "x", count: 1, destination: destination)
    }

    private func link(_ keys: [String: Any]) throws -> NotificationLink {
        try #require(NotificationLink(userInfo: ["agency": keys]))
    }

    @Test("it starts on the desk, with nothing to open, on the chart page")
    func start() {
        let router = ShellRouter()
        #expect(router.tab == .desk)
        #expect(router.openItemID == nil)
        #expect(router.organizationPage == .orgChart)
    }

    @Test("a banner for a request opens that request on the desk, from any screen")
    func requestBanner() {
        let router = ShellRouter()
        router.tab = .results
        router.open(banner(.request, .desk(itemID: "acme:app:r-1")))
        #expect(router.tab == .desk)
        #expect(router.openItemID == "acme:app:r-1")
    }

    @Test("a banner for a decision opens the desk and leaves no request open")
    func decisionBanner() {
        let router = ShellRouter()
        router.openItemID = "acme:app:r-1"
        router.tab = .devices
        router.open(banner(.decision, .desk(itemID: nil)))
        #expect(router.tab == .desk)
        #expect(router.openItemID == nil)
    }

    @Test("a banner for a participant or blocked work opens the participants page of the organization tab, whichever page was open")
    func peopleBanner() {
        let router = ShellRouter()
        router.organizationPage = .orgChart
        router.open(banner(.participant, .people))
        #expect(router.tab == .organization)
        #expect(router.organizationPage == .people)
    }

    @Test("a banner for a device opens the devices screen")
    func deviceBanner() {
        let router = ShellRouter()
        router.open(banner(.device, .devices))
        #expect(router.tab == .devices)
    }

    @Test("a tapped notification for a request opens it; for a device, the devices screen; for a decision, the desk")
    func notificationLinks() throws {
        let router = ShellRouter()
        router.open(try link(["org": "acme", "team": "app", "request": "r-7"]))
        #expect(router.tab == .desk)
        #expect(router.openItemID == "acme:app:r-7")
        router.open(try link(["type": "device"]))
        #expect(router.tab == .devices)
        router.tab = .results
        router.open(try link(["type": "decision", "org": "acme", "team": "app", "id": "dc-1"]))
        #expect(router.tab == .desk)
    }

    @Test("only the devices screen shows a device that asks by itself, so only there it makes no banner")
    func suppressed() {
        let router = ShellRouter()
        for tab in AppTab.allCases {
            router.tab = tab
            #expect(router.suppressedNewsKinds == (tab == .devices ? [.device] : []), "\(tab)")
        }
    }

    @Test("every tab has a title and a symbol")
    func tabs() {
        for tab in AppTab.allCases {
            #expect(!tab.title.isEmpty && !tab.symbol.isEmpty)
        }
        #expect(OrganizationPage.allCases.map(\.title) == [Ja.Tabs.orgChart, Ja.Tabs.people])
    }
}
