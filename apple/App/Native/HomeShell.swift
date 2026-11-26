import AgencyKit
import AgencyModel
import AgencyUI
import SwiftUI

/// The app for one home node: a tab for each native screen.
struct HomeShell: View {
    let homeNode: HomeNodeURL
    let push: PushCoordinator
    let onChangeHomeNode: () -> Void

    /// The tabs, in the order of the bar. Each has its own view in `screen(_:session:)`.
    enum Tab: CaseIterable, Hashable {
        case desk, overview, people, results, devices

        var title: String {
            switch self {
            case .desk: Ja.Tabs.desk
            case .overview: Ja.Tabs.overview
            case .people: Ja.Tabs.people
            case .results: Ja.Tabs.results
            case .devices: Ja.Tabs.devices
            }
        }

        var symbol: String {
            switch self {
            case .desk: "tray.full"
            case .overview: "square.grid.2x2"
            case .people: "person.2"
            case .results: "chart.line.uptrend.xyaxis"
            case .devices: "iphone"
            }
        }
    }

    @State private var session: NativeSession?
    @State private var flow: AccessSignInFlow?
    @State private var tab: Tab = .desk
    @State private var openItemID: String?
    @Environment(\.scenePhase) private var scenePhase
    @State private var showsSettings = false
    @State private var triedSignIn = false

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
                .overlay(alignment: .top) {
                    NewsBannerStack(
                        banners: session.news.banners,
                        onOpen: { open($0, session: session) },
                        onClose: { session.news.dismiss() }
                    )
                    .animation(.default, value: session.news.banners)
                }
                .task(id: scenePhase) { await watchNews(session) }
                .task(id: session.news.banners) { await dismissBannersSoon(session) }
                .onChange(of: tab) { _, tab in
                    // The devices screen shows a device that asks to be added by itself.
                    session.news.suppressedKinds = tab == .devices ? [.device] : []
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
        .onChange(of: push.registration) { _, registration in
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

    /// The screen of a tab.
    @ViewBuilder
    private func screen(_ tab: Tab, session: NativeSession) -> some View {
        switch tab {
        case .desk:
            desk(session)
        case .overview:
            OverviewScreen(
                store: session.overview, onOpenDesk: { self.tab = .desk },
                onSignInAgain: { Task { await session.signInAgain() } }
            )
        case .people:
            PeopleScreen(
                store: session.people, onOpenDesk: { self.tab = .desk },
                onSignInAgain: { Task { await session.signInAgain() } }
            )
        case .results:
            ResultsScreen(costs: session.costs, evaluation: session.evaluation, onSignInAgain: { Task { await session.signInAgain() } })
        case .devices:
            DevicesScreen(store: session.devices, pairing: session.pairing, notice: session.keyProblem.map { _ in Ja.Device.noKey })
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
        takeLink()
    }

    /// Reads the news while the app is in front: now, and every 30 seconds.
    private func watchNews(_ session: NativeSession) async {
        guard scenePhase == .active else { return }
        while !Task.isCancelled {
            await session.news.poll()
            try? await Task.sleep(for: .seconds(30))
        }
    }

    /// A banner stays a few seconds, long enough to read and not long enough to be in the way.
    private func dismissBannersSoon(_ session: NativeSession) async {
        guard !session.news.banners.isEmpty else { return }
        try? await Task.sleep(for: .seconds(7))
        if !Task.isCancelled { session.news.dismiss() }
    }

    /// A banner was tapped: go where it leads.
    private func open(_ banner: NewsBanner, session: NativeSession) {
        session.news.dismiss()
        switch banner.destination {
        case .desk(let itemID):
            tab = .desk
            openItemID = itemID
        case .people:
            tab = .people
        case .devices:
            tab = .devices
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
        flow?.forget()
        onChangeHomeNode()
    }
}
