import AgencyKit
import AgencyModel
import AgencyUI
import SwiftUI

/// The app for one home node: the desk as native screens, and the screens that are still web pages
/// behind the other tabs. One web view serves all of them and shows the page of the tab selected.
struct HomeShell: View {
    let homeNode: HomeNodeURL
    let push: PushCoordinator
    let onChangeHomeNode: () -> Void

    /// The tabs, in the order of the bar. A screen that is native has its own view in `screen(_:session:)`
    /// and no `webPath`; one that is still a web page has the page's path and is served by the shared
    /// web view. Making a screen native is a change to its case here and to the switch below, and to
    /// nothing else in the shell.
    enum Tab: CaseIterable, Hashable {
        case desk, overview, people, costs, devices

        var title: String {
            switch self {
            case .desk: Ja.Tabs.desk
            case .overview: Ja.Tabs.overview
            case .people: Ja.Tabs.people
            case .costs: Ja.Tabs.costs
            case .devices: Ja.Tabs.devices
            }
        }

        var symbol: String {
            switch self {
            case .desk: "tray.full"
            case .overview: "square.grid.2x2"
            case .people: "person.2"
            case .costs: "yensign.circle"
            case .devices: "iphone"
            }
        }

        /// The web client's page for this tab, or `nil` when the screen is native.
        var webPath: String? {
            switch self {
            case .desk: nil
            case .overview: "/overview"
            case .people: "/people"
            case .costs: "/costs"
            case .devices: "/devices"
            }
        }
    }

    @State private var session: NativeSession?
    @State private var flow: AccessSignInFlow?
    @State private var web: NodeWebModel?
    @State private var tab: Tab = .desk
    @State private var openItemID: String?
    @State private var showsSettings = false
    @State private var triedSignIn = false
    @State private var accessTeamHost: String?

    var body: some View {
        Group {
            if let session {
                TabView(selection: $tab) {
                    ForEach(Tab.allCases, id: \.self) { tab in
                        screen(tab, session: session)
                            .tabItem { Label(tab.title, systemImage: tab.symbol) }
                            .tag(tab)
                    }
                }
                .sheet(isPresented: $showsSettings) {
                    SettingsSheet(
                        host: homeNode.host,
                        onChangeHomeNode: {
                            showsSettings = false
                            Task { await leave() }
                        },
                        onResetKey: resetKey,
                        onOpenOrgChart: openOrgChart
                    )
                }
            } else {
                ProgressView()
            }
        }
        .task { start() }
        .onChange(of: tab) { _, tab in openWeb(tab) }
        .onChange(of: push.registration) { _, registration in
            web?.updatePush(registration)
            if let registration, let session { Task { await session.registerPush(registration) } }
        }
        .onChange(of: push.pendingLink) { takeLink() }
    }

    // MARK: Tabs

    private func desk(_ session: NativeSession) -> some View {
        DeskScreen(
            store: session.desk, pairing: session.pairing, notice: notice(session), openItemID: $openItemID,
            onSignInAgain: { Task { await session.signInAgain() } },
            onSettings: { showsSettings = true }
        )
        .onChange(of: session.desk.waitingCount) { _, count in push.setBadge(count) }
        .onChange(of: session.desk.loading) { _, loading in loaded(loading, session) }
    }

    /// The screen of a tab: a native view where there is one, otherwise the web page.
    @ViewBuilder
    private func screen(_ tab: Tab, session: NativeSession) -> some View {
        switch tab {
        case .desk:
            desk(session)
        case .overview, .people, .costs, .devices:
            webScreen(tab)
        }
    }

    @ViewBuilder
    private func webScreen(_ tab: Tab) -> some View {
        if self.tab == tab, let web {
            WebTabView(model: web, onChangeHomeNode: { Task { await leave() } })
        } else {
            Color.clear
        }
    }

    private func notice(_ session: NativeSession) -> String? {
        [session.keyProblem.map { _ in Ja.Device.noKey }, session.signInProblem].compactMap { $0 }.first
    }

    // MARK: Behaviour

    private func start() {
        guard session == nil else { return }
        let tokens = AccessTokenStore(host: homeNode.host)
        let flow = AccessSignInFlow(homeNode: homeNode, tokens: tokens)
        self.flow = flow
        let session = NativeSession(homeNode: homeNode, tokens: tokens, signIn: flow)
        self.session = session
        accessTeamHost = AccessTeamStore(host: homeNode.host).host
        takeLink()
        Task { await learnAccessTeam(session) }
    }

    /// Asks the node where it sends a browser that is not signed in, and pins that Access team.
    private func learnAccessTeam(_ session: NativeSession) async {
        guard let team = await session.discoverAccessTeam() else { return }
        AccessTeamStore(host: homeNode.host).save(team)
        accessTeamHost = team
        web?.pinAccessTeam(team)
    }

    private func openWeb(_ tab: Tab) {
        guard let path = tab.webPath, let flow else { return }
        if let web {
            web.openPath(path)
        } else {
            web = NodeWebModel(
                homeNode: homeNode, push: push.registration, accessSignIn: flow, accessTeamHost: accessTeamHost, path: path
            )
        }
    }

    /// A notification was tapped: open the request on the desk.
    private func takeLink() {
        guard session != nil, let link = push.takePendingLink() else { return }
        tab = .desk
        openItemID = DeskItem.key(org: link.org, team: link.team, request: link.request)
    }

    private func loaded(_ loading: DeskStore.Loading, _ session: NativeSession) {
        switch loading {
        case .loaded:
            push.noteSignedIn()
            if let registration = push.registration { Task { await session.registerPush(registration) } }
        case .failed:
            // The first launch has no token: ask for the sign-in once, without waiting for a tap.
            if session.desk.needsSignIn, !triedSignIn {
                triedSignIn = true
                Task { await session.signInAgain() }
            }
        default:
            break
        }
    }

    /// The org chart is not a tab (the bar holds five); it opens in the browser, which holds the Access
    /// session too, until a native chart exists.
    private func openOrgChart() {
        guard let url = URL(string: "/org", relativeTo: homeNode.url)?.absoluteURL else { return }
        showsSettings = false
        UIApplication.shared.open(url, options: [:], completionHandler: nil)
    }

    /// Makes this phone's key again, for when Face ID was changed and the old key can no longer sign.
    private func resetKey() {
        SecureEnclaveSigner.deleteKey(host: homeNode.host)
        showsSettings = false
        session = nil
        start()
    }

    /// The number on the icon and the sign-in belong to the node that is being left.
    private func leave() async {
        push.setBadge(0)
        await flow?.forget()
        onChangeHomeNode()
    }
}
